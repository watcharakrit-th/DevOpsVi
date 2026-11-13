//! Runs the real binary and sends it SIGTERM.

use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};

/// Kills the server if a test fails before it exits on its own.
struct Server(Child);

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

/// The status code of `GET path`, or None if nothing answers.
fn get(port: u16, path: &str) -> Option<u16> {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).ok()?;
    stream.set_read_timeout(Some(Duration::from_secs(2))).ok()?;
    write!(
        stream,
        "GET {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n"
    )
    .ok()?;
    let mut response = String::new();
    stream.read_to_string(&mut response).ok()?;
    response.split(' ').nth(1)?.parse().ok()
}

#[test]
fn sigterm_keeps_serving_through_the_drain_then_exits_cleanly() {
    let (app_port, ops_port) = (free_port(), free_port());
    let mut server = Server(
        Command::new(env!("CARGO_BIN_EXE_miniarcade"))
            .env("APP_ADDR", format!("127.0.0.1:{app_port}"))
            .env("OPS_ADDR", format!("127.0.0.1:{ops_port}"))
            .env("DRAIN_SECONDS", "2")
            .stdout(Stdio::null())
            .spawn()
            .unwrap(),
    );
    let started = Instant::now();
    while get(ops_port, "/livez") != Some(200) {
        assert!(
            started.elapsed() < Duration::from_secs(10),
            "server never came up"
        );
        thread::sleep(Duration::from_millis(50));
    }

    let kill = Command::new("kill")
        .args(["-TERM", &server.0.id().to_string()])
        .status()
        .unwrap();
    assert!(kill.success());
    let signalled = Instant::now();

    thread::sleep(Duration::from_millis(500));
    assert_eq!(
        get(app_port, "/nope"),
        Some(404),
        "app port serves while draining"
    );
    assert_eq!(
        get(ops_port, "/livez"),
        Some(200),
        "ops port serves while draining"
    );

    let exit = loop {
        if let Some(exit) = server.0.try_wait().unwrap() {
            break exit;
        }
        assert!(
            signalled.elapsed() < Duration::from_secs(10),
            "server never exited"
        );
        thread::sleep(Duration::from_millis(50));
    };
    assert!(exit.success(), "{exit}");
    let drained = signalled.elapsed();
    assert!(
        drained >= Duration::from_secs(2),
        "exited before the drain ended: {drained:?}"
    );
}
