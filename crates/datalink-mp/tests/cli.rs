//! Smoke tests of the built `datalink-mp` binary, for parity with the old CLI:
//! what a power user's script sees from `host` and `join --ticket`.

mod common;

use common::{free_port, poll_until, FakeDll};
use ipc_protocol::IpcResponse;
use iroh_transport::{Ticket, Transport};
use std::io::{BufRead, BufReader, Read};
use std::net::{TcpListener, TcpStream};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::mpsc;
use std::thread::JoinHandle;
use std::time::Duration;

/// How long the binary may take to create its Transport and print its Ticket.
const STARTUP_DEADLINE: Duration = Duration::from_secs(30);

/// How long the binary may take to give up and exit on a fatal error.
const EXIT_DEADLINE: Duration = Duration::from_secs(30);

/// How long a friend's Transport may take to see a connect on loopback.
const PEER_NOTICE_DEADLINE: Duration = Duration::from_secs(10);

/// A running `datalink-mp` process. Killed when dropped, so a failed assertion
/// leaves no Helper behind.
struct HelperProcess {
    child: Child,
    stdout_lines: mpsc::Receiver<String>,
    stderr: Option<JoinHandle<String>>,
}

impl HelperProcess {
    fn spawn(args: &[&str]) -> Self {
        Self::spawn_with_env(args, &[])
    }

    fn spawn_with_env(args: &[&str], envs: &[(&str, &str)]) -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_datalink-mp"))
            .args(args)
            // The developer's own settings must not leak into the test.
            .env_remove("SMAC_HELPER_PORT")
            .env_remove("SMAC_HELPER_LOG_FILE")
            .envs(envs.iter().copied())
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("the datalink-mp binary should start");

        let stdout = child.stdout.take().expect("stdout is piped");
        let (line_tx, stdout_lines) = mpsc::channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                if line_tx.send(line).is_err() {
                    break;
                }
            }
        });

        let mut stderr = child.stderr.take().expect("stderr is piped");
        let stderr = std::thread::spawn(move || {
            let mut text = String::new();
            let _ = stderr.read_to_string(&mut text);
            text
        });

        Self {
            child,
            stdout_lines,
            stderr: Some(stderr),
        }
    }

    /// The first line of standard output, which is the Helper's Ticket.
    ///
    /// None when the Helper could not create a Transport (sandboxed
    /// environments); the test then returns early.
    fn first_stdout_line(&mut self) -> Option<String> {
        match self.stdout_lines.recv_timeout(STARTUP_DEADLINE) {
            Ok(line) => Some(line),
            Err(mpsc::RecvTimeoutError::Timeout) => {
                panic!("datalink-mp printed nothing within {STARTUP_DEADLINE:?}")
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                let status = self.wait_for_exit();
                let stderr = self.stderr();
                if transport_unavailable(&stderr) {
                    return None;
                }
                panic!("datalink-mp exited ({status:?}) before printing a Ticket: {stderr}");
            }
        }
    }

    /// Wait for the process to exit by itself. None if it is still running at
    /// the deadline.
    fn wait_for_exit(&mut self) -> Option<ExitStatus> {
        poll_until(EXIT_DEADLINE, || {
            self.child.try_wait().expect("should poll the process")
        })
    }

    /// Everything the process wrote to standard error. Call after it exited.
    fn stderr(&mut self) -> String {
        self.stderr
            .take()
            .expect("stderr is read once")
            .join()
            .expect("stderr reader should not panic")
    }
}

impl Drop for HelperProcess {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// True when the binary failed because it could not create a Transport, which
/// is expected in sandboxed environments.
fn transport_unavailable(stderr: &str) -> bool {
    let unavailable = stderr.contains("Failed to create transport");
    if unavailable {
        eprintln!("Transport creation failed (expected in sandboxed environments): {stderr}");
    }
    unavailable
}

/// Connect a fake DLL to a Helper process that has printed its Ticket.
fn connect_fake_dll(ipc_port: u16) -> FakeDll {
    poll_until(STARTUP_DEADLINE, || FakeDll::connect(ipc_port).ok())
        .expect("the Helper should accept a DLL connection on its IPC port")
}

/// Assert that the Helper answers a handshake on `ipc_port` with the Ticket it printed.
fn assert_handshake_carries_ticket(ipc_port: u16, printed_ticket: &str) {
    match connect_fake_dll(ipc_port).handshake() {
        IpcResponse::HandshakeOk { our_ticket, .. } => assert_eq!(
            our_ticket, printed_ticket,
            "the handshake should carry the Ticket printed on standard output"
        ),
        other => panic!("expected HandshakeOk, got {other:?}"),
    }
}

#[test]
fn host_prints_a_parseable_ticket_first_and_answers_an_ipc_handshake() {
    let ipc_port = free_port();
    let mut helper = HelperProcess::spawn(&["host", "--port", &ipc_port.to_string()]);

    let Some(first_line) = helper.first_stdout_line() else {
        return;
    };
    Ticket::parse(&first_line).expect("the first line of standard output should be a Ticket");

    assert_handshake_carries_ticket(ipc_port, &first_line);
}

#[test]
fn host_prefers_smac_helper_port_over_the_port_flag() {
    let flag_port = free_port();
    let env_port = free_port();
    let mut helper = HelperProcess::spawn_with_env(
        &["host", "--port", &flag_port.to_string()],
        &[("SMAC_HELPER_PORT", &env_port.to_string())],
    );

    let Some(ticket) = helper.first_stdout_line() else {
        return;
    };

    assert_handshake_carries_ticket(env_port, &ticket);
    assert!(
        TcpStream::connect(("127.0.0.1", flag_port)).is_err(),
        "nothing should listen on the port given by --port when SMAC_HELPER_PORT is set"
    );
}

#[test]
fn no_subcommand_behaves_as_host() {
    let ipc_port = free_port();
    let mut helper =
        HelperProcess::spawn_with_env(&[], &[("SMAC_HELPER_PORT", &ipc_port.to_string())]);

    let Some(first_line) = helper.first_stdout_line() else {
        return;
    };
    Ticket::parse(&first_line).expect("the first line of standard output should be a Ticket");

    assert_handshake_carries_ticket(ipc_port, &first_line);
}

#[test]
fn host_exits_with_an_error_when_the_ipc_port_is_taken() {
    let holder = TcpListener::bind("127.0.0.1:0").expect("should bind a loopback port");
    let taken_port = holder.local_addr().unwrap().port();
    let mut helper = HelperProcess::spawn(&["host", "--port", &taken_port.to_string()]);

    let status = helper
        .wait_for_exit()
        .expect("a failed IPC bind should be fatal, but the Helper kept running");
    let stderr = helper.stderr();
    if transport_unavailable(&stderr) {
        return;
    }

    assert!(!status.success(), "expected an error exit, got {status:?}");
    assert!(
        stderr.contains("Failed to bind TCP listener"),
        "the error should say the IPC port could not be bound, got: {stderr}"
    );
}

#[test]
fn join_connects_to_a_friends_helper_and_answers_an_ipc_handshake() {
    let friend = match Transport::new() {
        Ok(friend) => friend,
        Err(e) => {
            eprintln!("Transport creation failed (expected in sandboxed environments): {e:?}");
            return;
        }
    };
    let ipc_port = free_port();
    let mut helper = HelperProcess::spawn(&[
        "join",
        "--port",
        &ipc_port.to_string(),
        "--ticket",
        friend.our_ticket(),
    ]);

    let Some(first_line) = helper.first_stdout_line() else {
        return;
    };
    let ticket =
        Ticket::parse(&first_line).expect("the first line of standard output should be a Ticket");

    let friend_sees_the_helper = poll_until(PEER_NOTICE_DEADLINE, || {
        friend
            .connected_peers()
            .contains(&ticket.addr().id)
            .then_some(())
    });
    assert!(
        friend_sees_the_helper.is_some(),
        "the friend should list the joining Helper as connected, got {:?}",
        friend.connected_peers()
    );

    assert_handshake_carries_ticket(ipc_port, &first_line);
}

#[test]
fn join_with_text_that_is_not_a_ticket_exits_with_an_error() {
    let ipc_port = free_port();
    let mut helper = HelperProcess::spawn(&[
        "join",
        "--port",
        &ipc_port.to_string(),
        "--ticket",
        "this is not a ticket",
    ]);

    let status = helper
        .wait_for_exit()
        .expect("join with a bad Ticket should exit, but the Helper kept running");
    let stderr = helper.stderr();
    if transport_unavailable(&stderr) {
        return;
    }

    assert!(!status.success(), "expected an error exit, got {status:?}");
    assert!(
        stderr.contains("Invalid ticket"),
        "the error should say the Ticket is invalid, got: {stderr}"
    );
}
