//! Smoke tests of the built `datalink-mp` binary, for parity with the old CLI:
//! what a power user's script sees from `host` and `join --ticket`.

mod common;

use common::{free_port, hold_port, http_get, note_transport_unavailable, poll_until, FakeDll};
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

/// How long a Helper that should stay up is watched for an exit.
const STAYS_UP_GRACE: Duration = Duration::from_secs(1);

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

    /// The launch URL, printed on standard output in UI mode.
    ///
    /// None when the Helper could not create a Transport (sandboxed
    /// environments); the test then returns early.
    fn launch_url(&mut self) -> Option<String> {
        loop {
            let line = self.first_stdout_line()?;
            if line.starts_with("http://") {
                return Some(line);
            }
        }
    }

    /// True if the process is still running once `grace` has passed. An exit
    /// is seen as soon as it happens.
    fn keeps_running_for(&mut self, grace: Duration) -> bool {
        poll_until(grace, || self.child.try_wait().expect("should poll the process")).is_none()
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

/// The UI-mode command line: page served on `ui_port`, DLL connections on `ipc_port`.
fn ui_args(ui_port: u16, ipc_port: u16) -> Vec<String> {
    vec![
        "--no-browser".into(),
        "--ui-port".into(),
        ui_port.to_string(),
        "--port".into(),
        ipc_port.to_string(),
    ]
}

/// Split a launch URL into the UI port and the token.
fn parse_launch_url(url: &str) -> (u16, String) {
    let rest = url
        .strip_prefix("http://127.0.0.1:")
        .unwrap_or_else(|| panic!("the launch URL should be on 127.0.0.1, got {url}"));
    let (port, query) = rest.split_once("/?t=").expect("the launch URL should carry ?t=");
    (port.parse().expect("the launch URL should name a port"), query.to_string())
}

/// Assert that the page is served at the launch URL and the token opens the API.
fn assert_page_and_status_are_served(launch_url: &str) -> serde_json::Value {
    let (ui_port, token) = parse_launch_url(launch_url);
    let page = poll_until(STARTUP_DEADLINE, || {
        std::panic::catch_unwind(|| http_get(ui_port, "/", &[])).ok()
    })
    .expect("the page should be served");
    assert_eq!(page.status, 200);
    assert!(page.body.contains("datalink-mp"));

    let status = http_get(ui_port, "/api/status", &[("X-Token", &token)]);
    assert_eq!(status.status, 200);
    status.json()
}

#[test]
fn test_no_subcommand_starts_the_ui_and_prints_the_launch_url() {
    let (ui_port, ipc_port) = (free_port(), free_port());
    let args = ui_args(ui_port, ipc_port);
    let mut helper = HelperProcess::spawn(&args.iter().map(String::as_str).collect::<Vec<_>>());

    let Some(launch_url) = helper.launch_url() else {
        return;
    };

    let status = assert_page_and_status_are_served(&launch_url);
    assert_eq!(status["ipc_port"], ipc_port);
    assert_eq!(parse_launch_url(&launch_url).0, ui_port);
    // The DLL side is served too.
    let printed = status["ticket"].as_str().unwrap().to_string();
    assert_handshake_carries_ticket(ipc_port, &printed);
}

#[test]
fn test_ui_port_flag_wins_over_smac_ui_port() {
    // The port in the environment stays taken for the whole test: a Helper
    // that preferred it over the flag could not start.
    let (_holder, env_port) = hold_port();
    let (flag_port, ipc_port) = (free_port(), free_port());
    let args = ui_args(flag_port, ipc_port);
    let mut helper = HelperProcess::spawn_with_env(
        &args.iter().map(String::as_str).collect::<Vec<_>>(),
        &[("SMAC_UI_PORT", &env_port.to_string())],
    );

    let Some(launch_url) = helper.launch_url() else {
        return;
    };

    assert_eq!(parse_launch_url(&launch_url).0, flag_port);
    assert_page_and_status_are_served(&launch_url);
}

#[test]
fn test_smac_ui_port_chooses_the_ui_port() {
    let (env_port, ipc_port) = (free_port(), free_port());
    let mut helper = HelperProcess::spawn_with_env(
        &["--no-browser", "--port", &ipc_port.to_string()],
        &[("SMAC_UI_PORT", &env_port.to_string())],
    );

    let Some(launch_url) = helper.launch_url() else {
        return;
    };

    assert_eq!(parse_launch_url(&launch_url).0, env_port);
    assert_page_and_status_are_served(&launch_url);
}

#[test]
fn test_ui_mode_prints_its_name_version_and_how_to_quit() {
    let (ui_port, ipc_port) = (free_port(), free_port());
    let args = ui_args(ui_port, ipc_port);
    let mut helper = HelperProcess::spawn(&args.iter().map(String::as_str).collect::<Vec<_>>());

    let Some(first_line) = helper.first_stdout_line() else {
        return;
    };
    assert_eq!(first_line, format!("datalink-mp {}", env!("CARGO_PKG_VERSION")));
    let Some(_) = helper.launch_url() else {
        return;
    };
    let quit_line = helper.first_stdout_line().expect("a line on how to quit");
    assert!(quit_line.contains("Ctrl+C"), "got: {quit_line}");
    assert!(
        quit_line.contains("Quit on the page"),
        "the line should mention the Quit button, got: {quit_line}"
    );
}

#[test]
fn test_ui_mode_exits_with_status_0_after_quit() {
    let (ui_port, ipc_port) = (free_port(), free_port());
    let args = ui_args(ui_port, ipc_port);
    let mut helper = HelperProcess::spawn(&args.iter().map(String::as_str).collect::<Vec<_>>());
    let Some(launch_url) = helper.launch_url() else {
        return;
    };
    let (ui_port, token) = parse_launch_url(&launch_url);
    // The game is open when the player quits.
    let mut dll = poll_until(STARTUP_DEADLINE, || FakeDll::connect(ipc_port).ok())
        .expect("the Helper should accept a DLL connection on its IPC port");
    dll.handshake();

    let answer = common::http_request(
        ui_port,
        "POST",
        "/api/quit",
        &[("X-Token", &token), ("Content-Type", "application/json")],
    );

    assert_eq!(answer.status, 204, "Quit should be answered before the Helper goes away");
    let exit = helper
        .wait_for_exit()
        .expect("the Helper should exit after Quit, but it kept running");
    assert_eq!(exit.status.code(), Some(0), "stderr: {}", exit.stderr);
    // Release builds abort on a panic; this build only reports it here.
    assert!(!exit.stderr.contains("panicked"), "Quit panicked: {}", exit.stderr);
}

#[test]
fn test_ui_mode_never_writes_the_token_to_the_log_file() {
    let (ui_port, ipc_port) = (free_port(), free_port());
    let log_path = std::env::temp_dir().join(format!("datalink-mp-test-{ipc_port}.log"));
    let _ = std::fs::remove_file(&log_path);
    let args = ui_args(ui_port, ipc_port);
    let mut helper = HelperProcess::spawn_with_env(
        &args.iter().map(String::as_str).collect::<Vec<_>>(),
        &[("SMAC_HELPER_LOG_FILE", log_path.to_str().unwrap())],
    );

    let Some(launch_url) = helper.launch_url() else {
        return;
    };
    let (_, token) = parse_launch_url(&launch_url);
    assert_page_and_status_are_served(&launch_url);
    // A request with the wrong token is logged too, whatever the level.
    http_get(ui_port, "/api/status", &[("X-Token", "wrong")]);
    drop(helper);

    let log = std::fs::read_to_string(&log_path).unwrap_or_default();
    let _ = std::fs::remove_file(&log_path);
    assert!(!log.is_empty(), "the log file should have been written");
    assert!(!log.contains(&token), "the token was logged");
}

/// Assert that `helper`, started on an IPC port another program holds, exits
/// with an error that says so.
fn assert_exits_with_an_ipc_bind_error(mut helper: HelperProcess) {
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
fn test_host_exits_with_an_error_when_the_ipc_port_is_taken() {
    let (_holder, taken_port) = hold_port();

    let helper = HelperProcess::spawn(&["host", "--port", &taken_port.to_string()]);

    assert_exits_with_an_ipc_bind_error(helper);
}

#[test]
fn test_join_exits_with_an_error_when_the_ipc_port_is_taken() {
    // A friend to join, so that the taken port is all that is wrong.
    let friend = match Transport::new() {
        Ok(friend) => friend,
        Err(e) => return note_transport_unavailable(&e),
    };
    let (_holder, taken_port) = hold_port();

    let helper = HelperProcess::spawn(&[
        "join",
        "--port",
        &taken_port.to_string(),
        "--ticket",
        friend.our_ticket(),
    ]);

    assert_exits_with_an_ipc_bind_error(helper);
}

#[test]
fn test_ui_mode_keeps_running_with_a_banner_when_the_ipc_port_is_taken() {
    let (_holder, taken_port) = hold_port();
    let args = ui_args(free_port(), taken_port);
    let mut helper = HelperProcess::spawn(&args.iter().map(String::as_str).collect::<Vec<_>>());

    let Some(launch_url) = helper.launch_url() else {
        return;
    };

    let status = assert_page_and_status_are_served(&launch_url);
    assert_eq!(status["banners"], serde_json::json!(["ipc_port_in_use"]));
    // With no IPC server to run, the Helper must not take that for its end.
    assert!(
        helper.keeps_running_for(STAYS_UP_GRACE),
        "the Helper exited although its page was being served"
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
