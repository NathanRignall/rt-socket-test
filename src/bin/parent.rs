use std::{io::{Read, Write}, os::{fd::{FromRawFd, IntoRawFd, OwnedFd}, unix::net::UnixStream}, process::{Command, Stdio}};

use command_fds::{CommandFdExt, FdMapping};

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
            panic!("Failed to set scheduler");
        }
    }

    // create control and data sockets
    let (mut control_socket, child_control_socket) = UnixStream::pair().unwrap();
    let control_count = 0;

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
            panic!("Failed to set scheduler");
        }
    }

    // wait for the component to be ready
    let mut buffer = [0; 1];
    control_socket.read_exact(&mut buffer).unwrap();
    if buffer[0] != b'k' {
        panic!("Failed to start component");
    }

    let mut times = Vec::new();

    // now start looping to test the unix response time.
    let i = 0;
    loop {
        i+=1;

        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_micros() as u64;
        times.push((timestamp, control_count));

        control_socket.write_all(&[b'r', control_count]).unwrap();
        control_count += 1;

        let mut buffer = [0; 1];
        control_socket.read_exact(&mut buffer).unwrap();

        if buffer[0] != b'k' {
            panic!("Failed to run");
        }

        // finish after 10,000 iterations
        if i == 10000 {
            break;
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
