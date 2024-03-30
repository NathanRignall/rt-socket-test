use std::{
    io::{Read, Write},
    os::{
        fd::{FromRawFd, RawFd},
        unix::net::UnixStream,
    },
};

fn main() {
    println!("Hello, child!");

    // establish socket with parent
    let child_control_socket_fd: RawFd = unsafe { std::os::unix::io::FromRawFd::from_raw_fd(10) };
    let mut child_control_socket = unsafe { UnixStream::from_raw_fd(child_control_socket_fd) };
    let mut child_control_count: u8 = 0;

    // set non-blocking
    child_control_socket.set_nonblocking(true).unwrap();

    // sleep for 100ms to allow the parent to set up
    std::thread::sleep(std::time::Duration::from_millis(100));

    // acknowledge component init
    child_control_socket
        .write_all(&[b'k'])
        .expect("Failed to write to socket");

    // create vector to store timestamps
    let mut times = Vec::new();

    println!("Child ready to receive");

    loop {
        // wait for the parent to signal
        let mut buf = [0; 2];
        loop {
            match child_control_socket.read_exact(&mut buf) {
                Ok(_) => break,
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
                Err(e) => panic!("Failed to read from socket: {}", e),
            }
        }

        // check control count
        if buf[1] != child_control_count {
            panic!("Control count mismatch");
        }

        // store the timestamp
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_micros() as u64;
        times.push((timestamp, child_control_count));

        child_control_count += 1;

        match buf[0] {
            b'q' => {
                // break when the parent signals to quit
                break;
            }
            b'r' => {
                // signal the parent
                child_control_socket
                    .write_all(&[b'k'])
                    .expect("Failed to write to socket");
            }
            _ => (),
        }
    }

    println!("Goodbye, child! (Write)");

    // write the timestamps to a file
    let mut writer = csv::Writer::from_path("times-child.csv").unwrap();
    for (i, (timestamp, count)) in times.iter().enumerate() {
        writer
            .serialize((i, timestamp, count))
            .expect("Failed to write to file");
    }

    println!("Goodbye, child! (Done)");
}
