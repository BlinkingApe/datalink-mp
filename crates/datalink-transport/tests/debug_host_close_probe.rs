//! [DEBUG-hc01] Throwaway probe: what does a joiner's game get handed when the
//! host's game closes? Prints every queued message so two builds can be diffed.

use dp_types::{PlayerName, SessionDesc, GUID};
use datalink_transport::Transport;
use std::time::Duration;

fn hosted_game_with_a_named_joiner() -> Option<(Transport, Transport, u32)> {
    let host = Transport::new().ok()?;
    let joiner = Transport::new().ok()?;
    host.create_session(SessionDesc {
        guid_instance: GUID::new_random(),
        guid_application: GUID::new_random(),
        session_name: "Probe".to_string(),
        max_players: 4,
        ..Default::default()
    });
    host.create_player(PlayerName::default(), 0, vec![0x03]).expect("host player");
    let joiner_id = joiner.join_session_by_ticket(host.our_ticket()).expect("join");
    joiner.create_player(
        PlayerName { short_name: String::new(), long_name: "Joining Player".to_string() },
        0,
        vec![0x02],
    );
    std::thread::sleep(Duration::from_secs(2));
    while host.receive().is_some() {}
    while joiner.receive().is_some() {}
    Some((host, joiner, joiner_id))
}

fn dump(label: &str, joiner: &Transport) {
    std::thread::sleep(Duration::from_secs(2));
    let mut n = 0;
    while let Some(qm) = joiner.receive() {
        n += 1;
        let ty = (qm.from == 0)
            .then(|| qm.data.get(0..4).map(|b| u32::from_le_bytes(b.try_into().unwrap())))
            .flatten();
        eprintln!(
            "[DEBUG-hc01] {label} #{n} from={} to={} sys={:?} len={} data={:02x?}",
            qm.from, qm.to, ty.map(|t| format!("{t:#x}")), qm.data.len(), qm.data
        );
    }
    eprintln!("[DEBUG-hc01] {label} total={n} in_session={}", joiner.session_manager().in_session());
}

#[test]
fn debug_hc01_host_closes_once() {
    let Some((host, joiner, _)) = hosted_game_with_a_named_joiner() else { return };
    host.close_session();
    dump("once", &joiner);
}

#[test]
fn debug_hc01_host_closes_then_dll_disconnects() {
    let Some((host, joiner, _)) = hosted_game_with_a_named_joiner() else { return };
    host.close_session();
    host.close_session();
    dump("twice", &joiner);
}

#[test]
fn debug_hc01_host_helper_shuts_down() {
    let Some((host, joiner, _)) = hosted_game_with_a_named_joiner() else { return };
    host.close_session();
    host.shutdown();
    dump("shutdown", &joiner);
}

fn three_players_mid_game() -> Option<(Transport, Transport, u32, Transport, u32)> {
    let (host, alice, alice_id) = hosted_game_with_a_named_joiner()?;
    let bob = Transport::new().ok()?;
    let bob_id = bob.join_session_by_ticket(host.our_ticket()).expect("bob joins");
    bob.create_player(
        PlayerName { short_name: String::new(), long_name: "Second Joiner".to_string() },
        0,
        vec![0x02],
    );
    std::thread::sleep(Duration::from_secs(3));
    const HOSTS_PLAYER: u32 = 0x1001;
    for i in 0..5u8 {
        let _ = host.send(HOSTS_PLAYER, 0, vec![0xAA, i], true);
        let _ = alice.send(alice_id, bob_id, vec![0xBB, i], true);
        let _ = bob.send(bob_id, alice_id, vec![0xCC, i], true);
        let _ = alice.send(alice_id, HOSTS_PLAYER, vec![0xDD, i], true);
    }
    std::thread::sleep(Duration::from_secs(2));
    for t in [&host, &alice, &bob] {
        while t.receive().is_some() {}
    }
    Some((host, alice, alice_id, bob, bob_id))
}

#[test]
fn debug_hc01_three_players_host_closes() {
    let Some((host, alice, _, bob, _)) = three_players_mid_game() else { return };
    host.close_session();
    dump("three-alice", &alice);
    dump("three-bob", &bob);
}

/// Each joiner's game reacts to SESSIONLOST by closing too (CloseSession or quitting).
#[test]
fn debug_hc01_three_players_host_closes_joiners_close_after() {
    let Some((host, alice, _, bob, _)) = three_players_mid_game() else { return };
    host.close_session();
    dump("react-alice-1", &alice);
    dump("react-bob-1", &bob);
    alice.close_session();
    dump("react-alice-2", &alice);
    dump("react-bob-2", &bob);
    bob.close_session();
    dump("react-alice-3", &alice);
    dump("react-bob-3", &bob);
}

fn four_players_mid_game() -> Option<(Transport, Vec<(Transport, u32)>)> {
    let host = Transport::new().ok()?;
    host.create_session(SessionDesc {
        guid_instance: GUID::new_random(),
        guid_application: GUID::new_random(),
        session_name: "Probe4".to_string(),
        max_players: 7,
        ..Default::default()
    });
    host.create_player(PlayerName::default(), 0, vec![0x03]).expect("host player");
    let mut joiners = Vec::new();
    for name in ["Alice", "Bob", "Carol"] {
        let t = Transport::new().ok()?;
        let id = t.join_session_by_ticket(host.our_ticket()).expect("join");
        t.create_player(PlayerName { short_name: String::new(), long_name: name.to_string() }, 0, vec![0x02]);
        std::thread::sleep(Duration::from_secs(2));
        joiners.push((t, id));
    }
    std::thread::sleep(Duration::from_secs(3));
    const HOSTS_PLAYER: u32 = 0x1001;
    for i in 0..5u8 {
        let _ = host.send(HOSTS_PLAYER, 0, vec![0xAA, i], true);
        for (a, aid) in &joiners {
            for (_, bid) in &joiners {
                if aid != bid {
                    let _ = a.send(*aid, *bid, vec![0xBB, i], true);
                }
            }
            let _ = a.send(*aid, HOSTS_PLAYER, vec![0xDD, i], true);
        }
    }
    std::thread::sleep(Duration::from_secs(2));
    while host.receive().is_some() {}
    for (t, _) in &joiners {
        while t.receive().is_some() {}
    }
    Some((host, joiners))
}

#[test]
fn debug_hc01_four_players_host_closes() {
    let Some((host, joiners)) = four_players_mid_game() else { return };
    for (t, id) in &joiners {
        eprintln!("[DEBUG-hc01] four peers of {id:#x}: {}", t.connected_peers().len());
    }
    host.close_session();
    for (i, (t, _)) in joiners.iter().enumerate() {
        dump(&format!("four-j{i}"), t);
    }
}

#[test]
fn debug_hc01_four_players_host_closes_joiners_close_after() {
    let Some((host, joiners)) = four_players_mid_game() else { return };
    host.close_session();
    for (i, (t, _)) in joiners.iter().enumerate() {
        dump(&format!("react4-j{i}-a"), t);
    }
    for (k, (closer, _)) in joiners.iter().enumerate() {
        closer.close_session();
        for (i, (t, _)) in joiners.iter().enumerate() {
            dump(&format!("react4-after{k}-j{i}"), t);
        }
    }
}

/// The joiners' games keep sending for a moment after the host closes, as a
/// real game does until it handles SESSIONLOST. Record each Send's result.
#[test]
fn debug_hc01_four_players_joiners_send_after_host_closes() {
    let Some((host, joiners)) = four_players_mid_game() else { return };
    const HOSTS_PLAYER: u32 = 0x1001;
    host.close_session();
    for phase in ["immediately", "after-1s", "after-3s"] {
        match phase {
            "after-1s" => std::thread::sleep(Duration::from_secs(1)),
            "after-3s" => std::thread::sleep(Duration::from_secs(2)),
            _ => {}
        }
        for (i, (a, aid)) in joiners.iter().enumerate() {
            let mut targets = vec![("host", HOSTS_PLAYER), ("all", 0)];
            for (j, (_, bid)) in joiners.iter().enumerate() {
                if aid != bid {
                    targets.push((["j0", "j1", "j2"][j], *bid));
                }
            }
            for (name, to) in targets {
                let r = a.send(*aid, to, vec![0xEE, 1], true);
                eprintln!("[DEBUG-hc01] sendafter {phase} j{i}->{name}: {:?}", r.map_err(|e| e.to_string()));
            }
        }
    }
    for (i, (t, _)) in joiners.iter().enumerate() {
        dump(&format!("sendafter-j{i}"), t);
    }
}

fn dump_for(label: &str, joiners: &[(Transport, u32)], secs: u64) {
    let deadline = std::time::Instant::now() + Duration::from_secs(secs);
    let mut n = vec![0; joiners.len()];
    while std::time::Instant::now() < deadline {
        for (i, (t, _)) in joiners.iter().enumerate() {
            while let Some(qm) = t.receive() {
                n[i] += 1;
                eprintln!(
                    "[DEBUG-hc01] {label}-j{i} #{} from={} to={} len={} data={:02x?}",
                    n[i], qm.from, qm.to, qm.data.len(), qm.data
                );
            }
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    for (i, (t, _)) in joiners.iter().enumerate() {
        eprintln!("[DEBUG-hc01] {label}-j{i} total={} peers={} in_session={}", n[i], t.connected_peers().len(), t.session_manager().in_session());
    }
}

#[test]
fn debug_hc01_four_players_host_closes_then_helper_shuts_down() {
    let Some((host, joiners)) = four_players_mid_game() else { return };
    host.close_session();
    host.shutdown();
    dump_for("hshut", &joiners, 12);
}

#[test]
fn debug_hc01_four_players_host_helper_vanishes_without_closing() {
    let Some((host, joiners)) = four_players_mid_game() else { return };
    drop(host);
    dump_for("hdrop", &joiners, 12);
}
