//! The running Helper, driven the way the game's DLL drives it: a Helper is
//! started inside the test process on a free IPC port, and a fake DLL talks to
//! it over TCP. Assertions are on IPC responses only.

mod common;

use common::{hold_port, note_transport_unavailable, FakeDll};
use datalink_mp::{Config, Helper, StartError};
use ipc_protocol::{IpcRequest, IpcResponse, PROTOCOL_VERSION};
use iroh_transport::{Ticket, TransportOptions};

/// The configuration the tests start a Helper with. Port 0 picks a free IPC port.
fn test_config(ipc_port: u16) -> Config {
    Config {
        // No page shows the self-check here, so any folder does.
        game_folder: std::env::temp_dir(),
        ipc_port,
        transport_options: TransportOptions::default(),
        ui: None,
    }
}

/// Start a Helper on a free IPC port.
///
/// None when a Transport cannot be created (sandboxed environments); the test
/// then returns early.
fn start_helper() -> Option<Helper> {
    match datalink_mp::start(test_config(0)) {
        Ok(helper) => Some(helper),
        Err(StartError::Transport(e)) => {
            note_transport_unavailable(&e);
            None
        }
        Err(e) => panic!("the Helper should start on a free IPC port: {e:?}"),
    }
}

fn connect_fake_dll(helper: &Helper) -> FakeDll {
    FakeDll::connect(helper.ipc_port()).expect("the Helper should accept a DLL connection")
}

#[test]
fn test_handshake_is_answered_with_the_helpers_ticket() {
    let Some(helper) = start_helper() else {
        return;
    };
    let mut dll = connect_fake_dll(&helper);

    let (endpoint_id, our_ticket) = dll.handshake();

    let ticket = Ticket::parse(&our_ticket).expect("the handshake should carry a parseable Ticket");
    assert_eq!(
        *ticket.addr().id,
        endpoint_id,
        "the Ticket and the endpoint ID in the handshake should name the same Helper"
    );

    helper.shutdown();
}

#[test]
fn test_follow_up_request_is_answered_on_the_same_connection() {
    let Some(helper) = start_helper() else {
        return;
    };
    let mut dll = connect_fake_dll(&helper);
    let (_, handshake_ticket) = dll.handshake();

    match dll.request(&IpcRequest::GetOurTicket) {
        IpcResponse::StringValue { value } => assert_eq!(value, handshake_ticket),
        other => panic!("expected StringValue, got {other:?}"),
    }
    match dll.request(&IpcRequest::InSession) {
        IpcResponse::Bool { value } => assert!(!value, "a fresh Helper is not in a session"),
        other => panic!("expected Bool, got {other:?}"),
    }

    helper.shutdown();
}

#[test]
fn test_handshake_with_the_wrong_ipc_version_gets_an_error_reply() {
    let Some(helper) = start_helper() else {
        return;
    };
    let mut dll = connect_fake_dll(&helper);

    match dll.handshake_with_version(PROTOCOL_VERSION + 1) {
        IpcResponse::Error { .. } => {}
        other => panic!("expected Error, got {other:?}"),
    }

    helper.shutdown();
}

#[test]
fn test_new_dll_connection_is_served_after_the_previous_one_closes() {
    let Some(helper) = start_helper() else {
        return;
    };

    let mut first = connect_fake_dll(&helper);
    let (_, first_ticket) = first.handshake();
    drop(first);

    // The game was restarted: its DLL connects again.
    let mut second = connect_fake_dll(&helper);
    let (_, second_ticket) = second.handshake();
    assert_eq!(second_ticket, first_ticket);

    helper.shutdown();
}

#[test]
fn test_start_fails_with_an_ipc_bind_error_when_the_ipc_port_is_taken() {
    let (_holder, taken_port) = hold_port();

    match datalink_mp::start(test_config(taken_port)) {
        Err(StartError::IpcBind(_)) => {}
        Err(StartError::Transport(e)) => note_transport_unavailable(&e),
        Ok(_) => panic!("start should fail while another program holds the IPC port"),
        Err(e) => panic!("expected an IPC bind error, got {e:?}"),
    }
}
