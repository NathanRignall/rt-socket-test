use std::{io::{Read, Write}, os::{fd::{FromRawFd, IntoRawFd, OwnedFd}, unix::net::UnixStream}, process::{Command, Stdio}};

use command_fds::{CommandFdExt, FdMapping};
use libc::cpu_set_t;

fn main() {
    println!("Hello, parent!");

    // use libc to set the process sechdeuler to SCHEDULER FFIO
    unsafe {
        let ret = libc::sched_setscheduler(
            0,
            libc::SCHED_FIFO,
            &libc::sched_param {
                sched_priority: 99,
            },
        );
        if ret != 0 {
            println!("Failed to set scheduler");
        }
    }

    // use libc to set the process core affinity to core 1
    let mut cpu_set: cpu_set_t = unsafe { std::mem::zeroed() };
    unsafe {
        libc::CPU_ZERO(&mut cpu_set);
        libc::CPU_SET(1, &mut cpu_set);
        let ret = libc::sched_setaffinity(0, std::mem::size_of_val(&cpu_set), &cpu_set);
        if ret != 0 {
            panic!("Failed to set affinity");
        }
    }

    // create control and data sockets
    let (mut control_socket, child_control_socket) = UnixStream::pair().unwrap();
    let mut control_count = 0;

    // set non-blocking
    control_socket.set_nonblocking(true).unwrap();

    // create fds for the child process
    let child_control_socket_fd = child_control_socket.into_raw_fd();

    // spawn the child process
    let binary_path = format!("target/release/child");
    let mut command = Command::new(binary_path);
    command
        .fd_mappings(vec![
            FdMapping {
                child_fd: 10,
                parent_fd: unsafe { OwnedFd::from_raw_fd(child_control_socket_fd) },
            },
        ])
        .unwrap();
    
    // redirect the child's stderr to the parent's stderr
    let child = command
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap();

    // set the scheduler for the child process
    unsafe {
        let ret = libc::sched_setscheduler(
            child.id() as libc::pid_t,
            libc::SCHED_FIFO,
            &libc::sched_param {
                sched_priority: 99,
            },
        );
        if ret != 0 {
            println!("Failed to set scheduler");
        }
    }

    // set the core affinity for the child process to core 2
    let mut cpu_set: cpu_set_t = unsafe { std::mem::zeroed() };
    unsafe {
        libc::CPU_ZERO(&mut cpu_set);
        libc::CPU_SET(2, &mut cpu_set);
        let ret = libc::sched_setaffinity(child.id() as libc::pid_t, std::mem::size_of_val(&cpu_set), &cpu_set);
        if ret != 0 {
            panic!("Failed to set affinity");
        }
    }

    // wait for the component to be ready
    let mut buffer = [0; 1];
    loop {
        match control_socket.read_exact(&mut buffer) {
            Ok(_) => break,
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {},
            Err(e) => panic!("Failed to read from socket: {}", e),
        }
    }
    if buffer[0] != b'k' {
        panic!("Failed to start component");
    }

    let mut times = Vec::new();

    let mut last_time;
    let period = std::time::Duration::from_micros(1_000_000 / 500 as u64);

    // now start looping to test the unix response time.
    let mut i = 0;
    loop {
        last_time = std::time::Instant::now();

        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_micros() as u64;
        times.push((timestamp, control_count));

        i+=1;

        println!("Parent: {}", i);
        
        control_socket.write_all(&[b'r', control_count]).unwrap();
        control_count += 1;

        let mut buffer = [0; 1];
        loop {
            match control_socket.read_exact(&mut buffer) {
                Ok(_) => break,
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {},
                Err(e) => panic!("Failed to read from socket: {}", e),
            }
        }

        if buffer[0] != b'k' {
            panic!("Failed to run");
        }

        // finish after 10,000 iterations
        if i == 10000 {
            control_socket.write_all(&[b'q', control_count]).unwrap();
            break;
        }

        let now = std::time::Instant::now();
        let duration = now.duration_since(last_time);

        if duration < period {
            std::thread::sleep(period - duration);
        } else {
            println!(
                "Warning: loop took longer than period {}us",
                duration.as_micros()
            );
        }
    }

    println!("Goodbye, parent! (Write)");

    let mut writer = csv::Writer::from_path("times-parent.csv").unwrap();
    for (i, (timestamp, count)) in times.iter().enumerate() {
        writer
            .serialize((i, timestamp, count))
            .expect("Failed to write to file");
    }

    println!("Goodbye, parent! (Done)");
}
