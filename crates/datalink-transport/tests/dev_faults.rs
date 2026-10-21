//! `DATALINK_FAULT=reset:N` over real local networking: the ordered writer
//! reopens the stream, replays what is in flight, and every message still
//! arrives once and in order. Its own test binary because the fault spec is
//! read from the process environment when a link is built.
#![cfg(feature = "dev-faults")]

use datalink_transport::Transport;
use dp_types::{PlayerName, SessionDesc, GUID};
use std::time::{Duration, Instant};

fn poll_until<T>(timeout: Duration, mut cond: impl FnMut() -> Option<T>) -> Option<T> {
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(v) = cond() {
            return Some(v);
        }
        if Instant::now() > deadline {
            return None;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn drain(host: &Transport, from: u32, into: &mut Vec<u32>) {
    while let Some(qm) = host.receive() {
        if qm.from == from && qm.data.len() == 4 {
            into.push(u32::from_le_bytes(qm.data[..4].try_into().unwrap()));
        }
    }
}

#[test]
fn reset_reopens_the_stream_and_delivery_stays_ordered_and_once() {
    // Before any Transport exists, so nothing else reads the environment.
    std::env::set_var("DATALINK_FAULT", "reset:20");
    let (Ok(host), Ok(joiner)) = (Transport::new(), Transport::new()) else {
        eprintln!("Transport creation failed (expected in sandboxed environments)");
        return;
    };
    host.create_session(SessionDesc {
        guid_instance: GUID::new_random(),
        guid_application: GUID::new_random(),
        session_name: "Reset".to_string(),
        max_players: 4,
        ..Default::default()
    });
    let name = |n: &str| PlayerName { short_name: n.to_string(), long_name: n.to_string() };
    let host_player = host.create_player(name("Host"), 0, vec![0x03]).expect("host player");
    let joiner_id = joiner.join_session_by_ticket(host.our_ticket()).expect("join");
    joiner.create_player(name("Joiner"), 0, vec![0x42]).expect("joiner player");

    const N: u32 = 200;
    for i in 0..N {
        joiner.send(joiner_id, host_player, i.to_le_bytes().to_vec(), true).expect("send");
    }
    let mut received = Vec::new();
    poll_until(Duration::from_secs(15), || {
        drain(&host, joiner_id, &mut received);
        (received.len() >= N as usize).then_some(())
    });
    // Let any duplicate from a replay show up.
    std::thread::sleep(Duration::from_millis(300));
    drain(&host, joiner_id, &mut received);
    assert_eq!(received, (0..N).collect::<Vec<_>>(), "each message once, in order, across the reset");
}
