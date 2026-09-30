//! The running Helper, driven the way the game's DLL drives it: a Helper is
//! started inside the test process on a free IPC port, and a fake DLL talks to
//! it over TCP. Assertions are on IPC responses only.

mod common;

use common::FakeDll;
use datalink_mp::{Config, Helper, StartError};
use ipc_protocol::{IpcRequest, IpcResponse, PROTOCOL_VERSION};
use iroh_transport::{Ticket, TransportOptions};
use std::net::TcpListener;

/// Start a Helper on a free IPC port.
///
/// None when a Transport cannot be created (sandboxed environments); the test
/// then returns early.
fn start_helper() -> Option<Helper> {
    match datalink_mp::start(Config {
        ipc_port: 0,
        transport_options: TransportOptions::default(),
    }) {
        Ok(helper) => Some(helper),
        Err(StartError::Transport(e)) => {
            eprintln!("Transport creation failed (expected in sandboxed environments): {e:?}");
            None
        }
        Err(e) => panic!("the Helper should start on a free IPC port: {e:?}"),
    }
}

fn connect_fake_dll(helper: &Helper) -> FakeDll {
    FakeDll::connect(helper.ipc_port()).expect("the Helper should accept a DLL connection")
}

/// Handshake and return the Ticket the Helper answered with.
fn handshake_ticket(dll: &mut FakeDll) -> String {
    match dll.handshake() {
        IpcResponse::HandshakeOk { our_ticket, .. } => our_ticket,
        other => panic!("expected HandshakeOk, got {other:?}"),
    }
}

#[test]
fn handshake_is_answered_with_the_helpers_ticket() {
    let Some(helper) = start_helper() else {
        return;
    };
    let mut dll = connect_fake_dll(&helper);

    let (endpoint_id, our_ticket) = match dll.handshake() {
        IpcResponse::HandshakeOk {
            endpoint_id,
            our_ticket,
        } => (endpoint_id, our_ticket),
        other => panic!("expected HandshakeOk, got {other:?}"),
    };

    let ticket = Ticket::parse(&our_ticket).expect("the handshake should carry a parseable Ticket");
    assert_eq!(
        *ticket.addr().id,
        endpoint_id,
        "the Ticket and the endpoint ID in the handshake should name the same Helper"
    );

    helper.shutdown();
}

#[test]
fn follow_up_request_is_answered_on_the_same_connection() {
    let Some(helper) = start_helper() else {
        return;
    };
    let mut dll = connect_fake_dll(&helper);
    let handshake_ticket = handshake_ticket(&mut dll);

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
fn handshake_with_the_wrong_ipc_version_gets_an_error_reply() {
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
fn a_new_dll_connection_is_served_after_the_previous_one_closes() {
    let Some(helper) = start_helper() else {
        return;
    };

    let mut first = connect_fake_dll(&helper);
    let first_ticket = handshake_ticket(&mut first);
    drop(first);

    // The game was restarted: its DLL connects again.
    let mut second = connect_fake_dll(&helper);
    assert_eq!(handshake_ticket(&mut second), first_ticket);

    helper.shutdown();
}

#[test]
fn start_fails_with_an_ipc_bind_error_when_the_ipc_port_is_taken() {
    let holder = TcpListener::bind("127.0.0.1:0").expect("should bind a loopback port");
    let taken_port = holder.local_addr().unwrap().port();

    match datalink_mp::start(Config {
        ipc_port: taken_port,
        transport_options: TransportOptions::default(),
    }) {
        Err(StartError::IpcBind(_)) => {}
        Err(StartError::Transport(e)) => {
            eprintln!("Transport creation failed (expected in sandboxed environments): {e:?}");
        }
        Ok(_) => panic!("start should fail while another program holds the IPC port"),
    }
}
