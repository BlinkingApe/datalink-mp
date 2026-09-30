//! Smoke tests of the built `datalink-mp` binary, for parity with the old CLI:
//! what a power user's script sees from `host` and `join --ticket`.

mod common;

use common::{free_port, hold_port, note_transport_unavailable, poll_until, FakeDll};
use iroh_transport::{Ticket, Transport};
use std::io::{BufRead, BufReader, Read};
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

/// How a `datalink-mp` process ended by itself.
struct Exit {
    status: ExitStatus,
    stderr: String,
}

impl Exit {
    /// True when the binary failed because it could not create a Transport,
    /// which is expected in sandboxed environments; the test then returns early.
    fn transport_unavailable(&self) -> bool {
        let unavailable = self.stderr.contains("Failed to create transport");
        if unavailable {
            note_transport_unavailable(&self.stderr);
        }
        unavailable
    }
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
                let exit = self
                    .wait_for_exit()
                    .expect("datalink-mp closed standard output without exiting");
                if exit.transport_unavailable() {
                    return None;
                }
                panic!(
                    "datalink-mp exited ({:?}) before printing a Ticket: {}",
                    exit.status, exit.stderr
                );
            }
        }
    }

    /// Wait for the process to exit by itself, and collect what it wrote to
    /// standard error. None if it is still running at the deadline.
    fn wait_for_exit(&mut self) -> Option<Exit> {
        let status = poll_until(EXIT_DEADLINE, || {
            self.child.try_wait().expect("should poll the process")
        })?;
        // The process is gone, so its end of the pipe is closed and the reader finishes.
        let stderr = self
            .stderr
            .take()
            .expect("the exit is collected once")
            .join()
            .expect("stderr reader should not panic");
        Some(Exit { status, stderr })
    }
}

impl Drop for HelperProcess {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Assert that the Helper answers a handshake on `ipc_port` with the Ticket it printed.
fn assert_handshake_carries_ticket(ipc_port: u16, printed_ticket: &str) {
    let mut dll = poll_until(STARTUP_DEADLINE, || FakeDll::connect(ipc_port).ok())
        .expect("the Helper should accept a DLL connection on its IPC port");
    let (_, our_ticket) = dll.handshake();
    assert_eq!(
        our_ticket, printed_ticket,
        "the handshake should carry the Ticket printed on standard output"
    );
}

#[test]
fn test_host_prints_a_parseable_ticket_first_and_answers_an_ipc_handshake() {
    let ipc_port = free_port();
    let mut helper = HelperProcess::spawn(&["host", "--port", &ipc_port.to_string()]);

    let Some(first_line) = helper.first_stdout_line() else {
        return;
    };
    Ticket::parse(&first_line).expect("the first line of standard output should be a Ticket");

    assert_handshake_carries_ticket(ipc_port, &first_line);
}

#[test]
fn test_host_prefers_smac_helper_port_over_the_port_flag() {
    // The port given by --port stays taken for the whole test: a Helper that
    // preferred it over SMAC_HELPER_PORT could not start.
    let (_holder, flag_port) = hold_port();
    let env_port = free_port();
    let mut helper = HelperProcess::spawn_with_env(
        &["host", "--port", &flag_port.to_string()],
        &[("SMAC_HELPER_PORT", &env_port.to_string())],
    );

    let Some(ticket) = helper.first_stdout_line() else {
        return;
    };

    assert_handshake_carries_ticket(env_port, &ticket);
}

#[test]
fn test_no_subcommand_behaves_as_host() {
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
fn test_host_exits_with_an_error_when_the_ipc_port_is_taken() {
    let (_holder, taken_port) = hold_port();
    let mut helper = HelperProcess::spawn(&["host", "--port", &taken_port.to_string()]);

    let exit = helper
        .wait_for_exit()
        .expect("a failed IPC bind should be fatal, but the Helper kept running");
    if exit.transport_unavailable() {
        return;
    }

    assert!(
        !exit.status.success(),
        "expected an error exit, got {:?}",
        exit.status
    );
    assert!(
        exit.stderr.contains("Failed to bind TCP listener"),
        "the error should say the IPC port could not be bound, got: {}",
        exit.stderr
    );
}

#[test]
fn test_join_connects_to_a_friend_and_answers_an_ipc_handshake() {
    let friend = match Transport::new() {
        Ok(friend) => friend,
        Err(e) => {
            note_transport_unavailable(&e);
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
fn test_join_with_text_that_is_not_a_ticket_exits_with_an_error() {
    let ipc_port = free_port();
    let mut helper = HelperProcess::spawn(&[
        "join",
        "--port",
        &ipc_port.to_string(),
        "--ticket",
        "this is not a ticket",
    ]);

    let exit = helper
        .wait_for_exit()
        .expect("join with a bad Ticket should exit, but the Helper kept running");
    if exit.transport_unavailable() {
        return;
    }

    assert!(
        !exit.status.success(),
        "expected an error exit, got {:?}",
        exit.status
    );
    assert!(
        exit.stderr.contains("Invalid ticket"),
        "the error should say the Ticket is invalid, got: {}",
        exit.stderr
    );
}
