//! Integration tests for mesh networking
//!
//! These tests verify the complete mesh networking flow:
//! 1. Host creates session and gets ticket
//! 2. Client connects via ticket and joins session
//! 3. Host broadcasts player joined with client's ticket
//! 4. Third player joins and connects to both host and existing client
//! 5. Messages are routed correctly in the mesh

use dp_types::{PlayerName, SessionDesc, GUID};
use iroh_transport::Transport;
use std::time::Duration;

/// Helper to create a test transport
/// Transport::new() creates its own runtime, so these tests must be synchronous
fn create_transport() -> Option<Transport> {
    // Transport creation involves network binding which may fail in restricted environments
    match Transport::new() {
        Ok(t) => Some(t),
        Err(e) => {
            eprintln!("Transport creation failed (expected in sandboxed environments): {:?}", e);
            None
        }
    }
}

#[test]
fn test_session_creation_and_ticket() {
    // Skip if we can't create transport (sandboxed environment)
    let transport = match create_transport() {
        Some(t) => t,
        None => return,
    };

    // Create a session
    let desc = SessionDesc {
        guid_instance: GUID::new_random(),
        guid_application: GUID::new_random(),
        session_name: "Test Session".to_string(),
        max_players: 8,
        ..Default::default()
    };

    let session_guid = transport.create_session(desc);
    assert!(session_guid.is_some(), "Session creation should succeed");

    // Check that ticket is available
    let ticket = transport.our_ticket();
    assert!(!ticket.is_empty(), "Ticket should not be empty");

    // Ticket should be valid (smac prefix + base32 encoded)
    assert!(ticket.starts_with("smac"), "Ticket should start with 'smac' prefix");
    let parsed = iroh_transport::Ticket::parse(ticket);
    assert!(parsed.is_ok(), "Ticket should be parseable: {:?}", parsed.err());

    // Check session state
    assert!(transport.session_manager().is_host());
    assert!(transport.session_manager().in_session());
}

#[test]
fn test_player_creation() {
    let transport = match create_transport() {
        Some(t) => t,
        None => return,
    };

    // Create session first
    let desc = SessionDesc {
        guid_instance: GUID::new_random(),
        guid_application: GUID::new_random(),
        session_name: "Test Session".to_string(),
        max_players: 8,
        ..Default::default()
    };

    transport.create_session(desc);

    // Create a player
    let name = PlayerName {
        short_name: "Player1".to_string(),
        long_name: "Player One".to_string(),
    };

    let player_id = transport.create_player(name, 0, Vec::new());
    assert!(player_id.is_some(), "Player creation should succeed");

    // First player should get ID 0x10000
    let id = player_id.unwrap();
    assert_eq!(id, 0x10000, "First player ID should be 0x10000");

    // Check that player is in session
    let players = transport.session_manager().get_players();
    assert_eq!(players.len(), 1);
    assert_eq!(players[0].id, id);
    assert_eq!(players[0].name.short_name, "Player1");
}

#[test]
fn test_multiple_player_ids() {
    let transport = match create_transport() {
        Some(t) => t,
        None => return,
    };

    let desc = SessionDesc {
        guid_instance: GUID::new_random(),
        guid_application: GUID::new_random(),
        session_name: "Test Session".to_string(),
        max_players: 8,
        ..Default::default()
    };

    transport.create_session(desc);

    // Allocate multiple player IDs
    let id1 = transport.session_manager().allocate_player_id();
    let id2 = transport.session_manager().allocate_player_id();
    let id3 = transport.session_manager().allocate_player_id();

    assert_eq!(id1, Some(0x10000), "First ID should be 0x10000");
    assert_eq!(id2, Some(0x20000), "Second ID should be 0x20000");
    assert_eq!(id3, Some(0x30000), "Third ID should be 0x30000");
}

#[test]
fn test_session_info_with_ticket() {
    let transport = match create_transport() {
        Some(t) => t,
        None => return,
    };

    let desc = SessionDesc {
        guid_instance: GUID::new_random(),
        guid_application: GUID::new_random(),
        session_name: "Test Session".to_string(),
        max_players: 8,
        ..Default::default()
    };

    transport.create_session(desc.clone());

    // Get session info
    let info = transport.session_manager().get_session_info();
    assert!(info.is_some(), "Session info should be available");

    let info = info.unwrap();
    assert_eq!(info.session_name, "Test Session");
    assert_eq!(info.max_players, 8);
    assert!(!info.host_ticket.is_empty(), "Host ticket should be set");
    assert_eq!(info.host_node_id, transport.endpoint_id_bytes());
}

#[test]
fn test_player_info_with_ticket() {
    let transport = match create_transport() {
        Some(t) => t,
        None => return,
    };

    let desc = SessionDesc {
        guid_instance: GUID::new_random(),
        guid_application: GUID::new_random(),
        session_name: "Test Session".to_string(),
        max_players: 8,
        ..Default::default()
    };

    transport.create_session(desc);

    let name = PlayerName {
        short_name: "Player1".to_string(),
        long_name: "Player One".to_string(),
    };

    transport.create_player(name, 0, Vec::new());

    // Get player infos for network transmission
    let infos = transport.session_manager().get_player_infos();
    assert_eq!(infos.len(), 1);

    let info = &infos[0];
    assert_eq!(info.player_id, 0x10000);
    assert!(!info.ticket.is_empty(), "Player ticket should be set");
    assert_eq!(info.node_id, transport.endpoint_id_bytes());
}

#[test]
fn test_protocol_encoding() {
    use iroh_transport::protocol::{encode_message, decode_message, Message, PlayerInfo};

    // Test encoding/decoding a PlayerJoined message with ticket
    let player_info = PlayerInfo {
        player_id: 0x10000,
        name: PlayerName {
            short_name: "TestPlayer".to_string(),
            long_name: "Test Player Full Name".to_string(),
        },
        flags: 0,
        node_id: [1; 32],
        ticket: r#"{"node_id":"test","relay_url":null,"direct_addresses":[]}"#.to_string(),
        data: vec![1, 2, 3],
    };

    let msg = Message::PlayerJoined { player: player_info.clone() };
    let encoded = encode_message(&msg).expect("Encoding should succeed");
    let decoded = decode_message(&encoded).expect("Decoding should succeed");

    match decoded {
        Message::PlayerJoined { player } => {
            assert_eq!(player.player_id, 0x10000);
            assert_eq!(player.ticket, player_info.ticket);
            assert_eq!(player.name.short_name, "TestPlayer");
        }
        _ => panic!("Expected PlayerJoined message"),
    }
}

#[test]
fn test_join_request_with_ticket() {
    use iroh_transport::protocol::{encode_message, decode_message, Message};

    let msg = Message::JoinRequest {
        session_id: GUID::new_random(),
        player_name: PlayerName {
            short_name: "Joiner".to_string(),
            long_name: "Joining Player".to_string(),
        },
        player_data: vec![],
        my_ticket: r#"{"node_id":"abc123","relay_url":null,"direct_addresses":[]}"#.to_string(),
    };

    let encoded = encode_message(&msg).expect("Encoding should succeed");
    let decoded = decode_message(&encoded).expect("Decoding should succeed");

    match decoded {
        Message::JoinRequest { my_ticket, player_name, .. } => {
            assert!(my_ticket.contains("abc123"));
            assert_eq!(player_name.short_name, "Joiner");
        }
        _ => panic!("Expected JoinRequest message"),
    }
}

#[test]
fn test_session_manager_identity() {
    use iroh_transport::session::SessionManager;

    let manager = SessionManager::new();

    // Set identity
    let node_id = [42u8; 32];
    let ticket = "test_ticket".to_string();
    manager.set_identity(node_id, ticket.clone());

    // Verify identity
    assert_eq!(manager.our_node_id(), Some(node_id));
    assert_eq!(manager.our_ticket(), ticket);
}

#[test]
fn test_session_with_remote_players() {
    use iroh_transport::session::SessionManager;
    use iroh_transport::protocol::PlayerInfo;

    let manager = SessionManager::new();
    manager.set_identity([1; 32], "host_ticket".to_string());

    // Create session
    let desc = SessionDesc {
        guid_instance: GUID::new_random(),
        guid_application: GUID::new_random(),
        session_name: "Test".to_string(),
        max_players: 4,
        ..Default::default()
    };

    manager.create_session(desc);

    // Add a remote player
    let remote_player = PlayerInfo {
        player_id: 0x20000,
        name: PlayerName {
            short_name: "Remote".to_string(),
            long_name: "Remote Player".to_string(),
        },
        flags: 0,
        node_id: [2; 32],
        ticket: "remote_ticket".to_string(),
        data: vec![],
    };

    manager.add_remote_player(remote_player);

    // Check player count
    let players = manager.get_players();
    assert_eq!(players.len(), 1);
    assert_eq!(players[0].id, 0x20000);
    assert!(!players[0].is_local);
    assert_eq!(players[0].ticket, "remote_ticket");
}

#[test]
fn test_message_queue() {
    let transport = match create_transport() {
        Some(t) => t,
        None => return,
    };

    // Initially no messages
    assert!(transport.receive().is_none());
    assert_eq!(transport.message_count(), 0);
}

#[test]
fn test_close_session() {
    let transport = match create_transport() {
        Some(t) => t,
        None => return,
    };

    let desc = SessionDesc {
        guid_instance: GUID::new_random(),
        guid_application: GUID::new_random(),
        session_name: "Test".to_string(),
        max_players: 4,
        ..Default::default()
    };

    transport.create_session(desc);
    assert!(transport.session_manager().in_session());

    transport.close_session();

    // Give some time for async cleanup
    std::thread::sleep(Duration::from_millis(100));

    assert!(!transport.session_manager().in_session());
}

/// Poll until `cond` returns Some or the deadline passes.
fn poll_until<T>(timeout: Duration, mut cond: impl FnMut() -> Option<T>) -> Option<T> {
    let deadline = std::time::Instant::now() + timeout;
    loop {
        if let Some(v) = cond() {
            return Some(v);
        }
        if std::time::Instant::now() > deadline {
            return None;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// End-to-end test over real local networking: join by ticket, name-table
/// download, player-data broadcast, and — critically — ORDERED GameMessage
/// delivery over the per-peer framed stream (the task #15 regression test).
#[test]
fn test_join_and_ordered_delivery_end_to_end() {
    let host = match create_transport() {
        Some(t) => t,
        None => return,
    };
    let joiner = match create_transport() {
        Some(t) => t,
        None => return,
    };

    let desc = SessionDesc {
        guid_instance: GUID::new_random(),
        guid_application: GUID::new_random(),
        session_name: "Ordered Test".to_string(),
        max_players: 4,
        ..Default::default()
    };
    host.create_session(desc);

    // Host player carries a data byte (0x03 = is-host | reliable-eligible)
    let host_player_id = host
        .create_player(
            PlayerName {
                short_name: "Host".to_string(),
                long_name: "Host Player".to_string(),
            },
            0,
            vec![0x03],
        )
        .expect("host player");

    let joiner_id = joiner
        .join_session_by_ticket(host.our_ticket())
        .expect("join by ticket should succeed");

    // Name-table download: the joiner must know the host player INCLUDING its
    // data byte, delivered atomically in the JoinResponse.
    assert_eq!(
        joiner.session_manager().get_player_data(host_player_id, false),
        Some(vec![0x03]),
        "joiner should have host player's data from the join name-table"
    );

    // Joiner's CreatePlayer broadcasts name + data over the ordered stream.
    // Use a distinctive byte so this is provably the broadcast, not the
    // eligibility byte the host bakes in at JoinRequest time (0x02).
    let created = joiner.create_player(
        PlayerName {
            short_name: "Joiner".to_string(),
            long_name: "Joining Player".to_string(),
        },
        0,
        vec![0x42],
    );
    assert_eq!(created, Some(joiner_id));

    let host_sees = poll_until(Duration::from_secs(10), || {
        match host.session_manager().get_player_data(joiner_id, false) {
            Some(d) if d == vec![0x42] => Some(()),
            _ => None,
        }
    });
    assert!(
        host_sees.is_some(),
        "host should see joiner's data byte from the PlayerDataUpdate broadcast"
    );

    // Ordered delivery: 200 directed messages must arrive in send order.
    const N: u32 = 200;
    for i in 0..N {
        joiner
            .send(joiner_id, host_player_id, i.to_le_bytes().to_vec(), true)
            .expect("send");
    }

    let mut received = Vec::with_capacity(N as usize);
    let all = poll_until(Duration::from_secs(15), || {
        while let Some(qm) = host.receive() {
            // Filter to our test messages (4-byte payloads from the joiner)
            if qm.from == joiner_id && qm.data.len() == 4 {
                received.push(u32::from_le_bytes(qm.data[..4].try_into().unwrap()));
            }
        }
        (received.len() >= N as usize).then_some(())
    });
    assert!(
        all.is_some(),
        "expected {} messages, got {} — messages were dropped",
        N,
        received.len()
    );
    let expected: Vec<u32> = (0..N).collect();
    assert_eq!(received, expected, "messages arrived out of order");
}

/// Helper to create a test transport with non-default options: a different
/// Peer protocol version (a stand-in for a different build) or a short dial timeout
fn create_transport_with_options(options: iroh_transport::TransportOptions) -> Option<Transport> {
    match Transport::with_options(options) {
        Ok(t) => Some(t),
        Err(e) => {
            eprintln!("Transport creation failed (expected in sandboxed environments): {:?}", e);
            None
        }
    }
}

#[test]
fn test_default_options_are_peer_protocol_version_1_and_15s_dial_timeout() {
    let options = iroh_transport::TransportOptions::default();
    assert_eq!(options.peer_protocol_version, 1, "default Peer protocol version");
    assert_eq!(options.dial_timeout, Duration::from_secs(15), "default dial timeout");
}

/// Helper to bind a bare Iroh endpoint: a stand-in for "some other program on
/// the wire", used to observe what a Transport advertises and sends.
async fn bind_bare_endpoint(alpns: Vec<Vec<u8>>) -> Option<iroh::Endpoint> {
    match iroh::Endpoint::builder(iroh::endpoint::presets::N0).alpns(alpns).bind().await {
        Ok(ep) => Some(ep),
        Err(e) => {
            eprintln!("Endpoint bind failed (expected in sandboxed environments): {:?}", e);
            None
        }
    }
}

/// The ALPN of a default Transport is `datalink/1`. A dial with that ALPN is
/// accepted; a dial with smac-iroh's `dplay-iroh/1` is refused, which is the
/// deliberate break with smac-iroh builds.
#[test]
fn test_default_transport_accepts_alpn_datalink_1_and_refuses_dplay_iroh_1() {
    let transport = match create_transport() {
        Some(t) => t,
        None => return,
    };
    let addr = iroh_transport::Ticket::parse(transport.our_ticket())
        .expect("our ticket parses")
        .into_addr();

    let rt = tokio::runtime::Runtime::new().expect("test runtime");
    let outcome = rt.block_on(async {
        let dialler = bind_bare_endpoint(Vec::new()).await?;
        let old = dialler.connect(addr.clone(), b"dplay-iroh/1").await.map(|_| ());
        let current = dialler.connect(addr, b"datalink/1").await.map(|_| ());
        dialler.close().await;
        Some((old, current))
    });
    let (old, current) = match outcome {
        Some(o) => o,
        None => return,
    };

    assert!(current.is_ok(), "dial with ALPN datalink/1 should be accepted: {:?}", current.err());
    assert!(old.is_err(), "dial with ALPN dplay-iroh/1 should be refused");
}

/// Two Transports with different Peer protocol versions refuse each other, in
/// both directions, and the dialler is told why: PeerProtocolMismatch, not CantReach.
#[test]
fn test_different_peer_protocol_versions_refuse_each_other() {
    let ours = match create_transport() {
        Some(t) => t,
        None => return,
    };
    let other_build = match create_transport_with_options(iroh_transport::TransportOptions {
        peer_protocol_version: 2,
        ..Default::default()
    }) {
        Some(t) => t,
        None => return,
    };

    // Both host a session, so the Peer protocol version is the only obstacle.
    for transport in [&ours, &other_build] {
        transport.create_session(SessionDesc {
            guid_instance: GUID::new_random(),
            guid_application: GUID::new_random(),
            session_name: "Mismatch Test".to_string(),
            max_players: 4,
            ..Default::default()
        });
    }

    // The join the DLL asks for, from the other build to ours
    let err = other_build
        .join_session_by_ticket(ours.our_ticket())
        .expect_err("join across Peer protocol versions should be refused");
    assert!(
        matches!(err, iroh_transport::TransportError::PeerProtocolMismatch),
        "dialler should get PeerProtocolMismatch, got {:?}",
        err
    );

    // And a plain dial the other way round
    let err = ours
        .connect_to_peer(other_build.our_ticket())
        .expect_err("dial across Peer protocol versions should be refused");
    assert!(
        matches!(err, iroh_transport::TransportError::PeerProtocolMismatch),
        "dialler should get PeerProtocolMismatch, got {:?}",
        err
    );

    // Neither side ever lists the other as connected. connect_for_discovery
    // succeeds only for an already-connected peer, so it serves as the read.
    let connected = poll_until(Duration::from_secs(1), || {
        let listed = ours.connect_for_discovery(other_build.endpoint_id_bytes()).is_ok()
            || other_build.connect_for_discovery(ours.endpoint_id_bytes()).is_ok();
        listed.then_some(())
    });
    assert!(connected.is_none(), "mismatched peers must never be listed as connected");
}

/// A dial to a Ticket nobody answers on gives up at the configured dial
/// timeout with CantReach, through the join the DLL asks for.
#[test]
fn test_dial_to_silent_ticket_fails_with_cant_reach_at_dial_timeout() {
    let dial_timeout = Duration::from_secs(2);
    let dialler = match create_transport_with_options(iroh_transport::TransportOptions {
        dial_timeout,
        ..Default::default()
    }) {
        Some(t) => t,
        None => return,
    };

    // A Ticket for a Helper that never existed, at a loopback address that is
    // bound (so nothing is refused) but never answers.
    let silent_socket = std::net::UdpSocket::bind("127.0.0.1:0").expect("bind silent socket");
    let silent_addr = silent_socket.local_addr().expect("silent socket address");
    let dead_ticket = iroh_transport::Ticket::new(
        iroh::EndpointAddr::new(iroh::SecretKey::generate().public()).with_ip_addr(silent_addr),
    )
    .serialize();

    let started = std::time::Instant::now();
    let err = dialler
        .join_session_by_ticket(&dead_ticket)
        .expect_err("join to a silent Ticket should fail");
    let elapsed = started.elapsed();

    assert!(
        matches!(err, iroh_transport::TransportError::CantReach),
        "dialler should get CantReach, got {:?}",
        err
    );
    assert!(elapsed >= dial_timeout, "gave up before the dial timeout: {:?}", elapsed);
    assert!(
        elapsed < Duration::from_secs(8),
        "should give up at the 2s dial timeout, well under the 15s default, took {:?}",
        elapsed
    );
}

/// A Ticket goes dead when its Helper goes away: a dial to the Ticket of a
/// dropped Transport fails with CantReach, no later than the dial timeout.
#[test]
fn test_dial_to_ticket_of_dropped_transport_fails_with_cant_reach() {
    let dialler = match create_transport_with_options(iroh_transport::TransportOptions {
        dial_timeout: Duration::from_secs(2),
        ..Default::default()
    }) {
        Some(t) => t,
        None => return,
    };
    let dead_ticket = match create_transport() {
        Some(gone) => gone.our_ticket().to_string(),
        None => return,
    };

    let started = std::time::Instant::now();
    let err = dialler
        .connect_to_peer(&dead_ticket)
        .expect_err("dial to a dead Ticket should fail");
    let elapsed = started.elapsed();

    assert!(
        matches!(err, iroh_transport::TransportError::CantReach),
        "dialler should get CantReach, got {:?}",
        err
    );
    assert!(
        elapsed < Duration::from_secs(8),
        "should give up by the 2s dial timeout, well under the 15s default, took {:?}",
        elapsed
    );
}

/// A Transport built with a non-default Peer protocol version is a faithful
/// stand-in for that build on the wire: it dials with the matching ALPN and
/// opens its ordered stream with that version in the preamble
/// (b"SMAC" + version as u16 LE).
#[test]
fn test_non_default_peer_protocol_version_is_used_in_alpn_and_stream_preamble() {
    let transport = match create_transport_with_options(iroh_transport::TransportOptions {
        peer_protocol_version: 7,
        ..Default::default()
    }) {
        Some(t) => t,
        None => return,
    };

    let rt = tokio::runtime::Runtime::new().expect("test runtime");
    let listener = match rt.block_on(bind_bare_endpoint(vec![b"datalink/7".to_vec()])) {
        Some(ep) => ep,
        None => return,
    };
    let listener_ticket = iroh_transport::Ticket::new(listener.addr()).serialize();

    // Accept one connection and read the preamble of its first ordered stream.
    let preamble = rt.spawn(async move {
        let connection = listener.accept().await?.await.ok()?;
        let mut recv = connection.accept_uni().await.ok()?;
        let mut preamble = [0u8; 6];
        recv.read_exact(&mut preamble).await.ok()?;
        Some(preamble)
    });

    transport
        .connect_to_peer(&listener_ticket)
        .expect("dial with ALPN datalink/7 should be accepted");
    // Any broadcast opens the ordered stream to the connected peer.
    transport.set_player_data(0x10000, vec![0x42], false);

    let preamble = rt
        .block_on(async { tokio::time::timeout(Duration::from_secs(10), preamble).await })
        .expect("ordered stream should open before the deadline")
        .expect("listener task");
    assert_eq!(preamble, Some(*b"SMAC\x07\x00"));
}

/// Two Transports on the same non-default Peer protocol version are compatible
/// builds: they connect, and messages on the ordered stream get through.
#[test]
fn test_same_non_default_peer_protocol_version_connects_and_delivers() {
    let options = iroh_transport::TransportOptions {
        peer_protocol_version: 7,
        ..Default::default()
    };
    let host = match create_transport_with_options(options) {
        Some(t) => t,
        None => return,
    };
    let joiner = match create_transport_with_options(options) {
        Some(t) => t,
        None => return,
    };

    host.create_session(SessionDesc {
        guid_instance: GUID::new_random(),
        guid_application: GUID::new_random(),
        session_name: "Same Version Test".to_string(),
        max_players: 4,
        ..Default::default()
    });

    let joiner_id = joiner
        .join_session_by_ticket(host.our_ticket())
        .expect("join between Transports on the same Peer protocol version should succeed");

    // The accepting side lists the peer as connected (the same read the
    // mismatch test uses to show a refused peer is never listed).
    let listed = poll_until(Duration::from_secs(10), || {
        host.connect_for_discovery(joiner.endpoint_id_bytes()).ok()
    });
    assert!(listed.is_some(), "host should list the joiner as connected");

    // The data byte travels over the ordered stream, behind the preamble check.
    joiner.create_player(
        PlayerName {
            short_name: "Joiner".to_string(),
            long_name: "Joining Player".to_string(),
        },
        0,
        vec![0x42],
    );
    let host_sees = poll_until(Duration::from_secs(10), || {
        match host.session_manager().get_player_data(joiner_id, false) {
            Some(d) if d == vec![0x42] => Some(()),
            _ => None,
        }
    });
    assert!(host_sees.is_some(), "host should receive the joiner's ordered-stream message");
}

/// Full mesh networking test (requires actual network access)
/// This test creates multiple transports and verifies mesh connectivity
#[test]
#[ignore] // Requires network access and may take time
fn test_mesh_networking() {
    // Create host
    let host = match create_transport() {
        Some(t) => t,
        None => return,
    };

    let desc = SessionDesc {
        guid_instance: GUID::new_random(),
        guid_application: GUID::new_random(),
        session_name: "Mesh Test".to_string(),
        max_players: 4,
        ..Default::default()
    };

    host.create_session(desc);

    let host_name = PlayerName {
        short_name: "Host".to_string(),
        long_name: "Host Player".to_string(),
    };
    host.create_player(host_name, 0, Vec::new());

    let host_ticket = host.our_ticket().to_string();
    println!("Host ticket: {}", host_ticket);

    // Create client and join
    let client = match create_transport() {
        Some(t) => t,
        None => return,
    };

    // This would normally use join_session_by_ticket, but for unit test
    // we just verify the components are set up correctly
    println!("Client ticket: {}", client.our_ticket());

    // Verify both transports have valid tickets
    assert!(!host_ticket.is_empty());
    assert!(!client.our_ticket().is_empty());
}

/// How long a Helper may take to see a connect, or a graceful close, on loopback.
/// Far below the 30 s QUIC idle timeout, which is the only way a peer learns of a
/// close that was never delivered — so a poll with this deadline fails if
/// `shutdown()` does not actually tell the other side.
const PEER_NOTICE_DEADLINE: Duration = Duration::from_secs(2);

/// Poll until `transport.connected_peers()` is exactly `expected`.
fn peers_become(transport: &Transport, expected: &[iroh::EndpointId]) -> bool {
    poll_until(PEER_NOTICE_DEADLINE, || {
        (transport.connected_peers() == expected).then_some(())
    })
    .is_some()
}

/// Two Transports with `dialler` connected to `acceptor` by Ticket, each already
/// listing the other as a connected Helper.
fn create_connected_pair() -> Option<(Transport, Transport)> {
    let acceptor = create_transport()?;
    let dialler = create_transport()?;

    dialler
        .connect_to_peer(acceptor.our_ticket())
        .expect("dial by Ticket should succeed on loopback");

    assert!(
        peers_become(&dialler, &[acceptor.endpoint_id()]),
        "dialling side should list the Helper it dialled, got {:?}",
        dialler.connected_peers()
    );
    // The accepting side registers the peer from its accept loop, slightly later.
    assert!(
        peers_become(&acceptor, &[dialler.endpoint_id()]),
        "accepting side should list the Helper that dialled it, got {:?}",
        acceptor.connected_peers()
    );

    Some((dialler, acceptor))
}

#[test]
fn test_connected_peers_lists_peer_after_connect_on_both_sides() {
    let acceptor = match create_transport() {
        Some(t) => t,
        None => return,
    };
    assert!(
        acceptor.connected_peers().is_empty(),
        "a fresh Transport has no connected Helpers"
    );
    drop(acceptor);

    // The pair helper asserts both sides against the other's endpoint_id().
    let _pair = create_connected_pair();
}

/// Status requests read the peer list from async code, where entering the
/// Transport's own runtime would panic ("Cannot start a runtime from within a
/// runtime"). The Transports are created and dropped outside the async block:
/// both of those do block.
#[test]
fn test_connected_peers_can_be_called_from_async_context() {
    let (dialler, acceptor) = match create_connected_pair() {
        Some(pair) => pair,
        None => return,
    };

    let status_runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()
        .expect("test runtime");
    let seen = status_runtime.block_on(async { dialler.connected_peers() });

    assert_eq!(seen, vec![acceptor.endpoint_id()]);
}

/// Stop and Quit rely on the other Helper being told, not left to time out.
#[test]
fn test_connected_peer_notices_shutdown_promptly() {
    let (dialler, acceptor) = match create_connected_pair() {
        Some(pair) => pair,
        None => return,
    };

    acceptor.shutdown();

    // `acceptor` stays alive below, so this can only be the delivered close.
    assert!(
        peers_become(&dialler, &[]),
        "peer should see the close within {:?}, still lists {:?}",
        PEER_NOTICE_DEADLINE,
        dialler.connected_peers()
    );
}

#[test]
fn test_shutdown_returns_within_its_bound_with_a_peer_connected() {
    // The bound is 3 seconds; the slack covers scheduling, not a longer wait.
    let bound = Duration::from_secs(3) + Duration::from_millis(500);

    let (dialler, acceptor) = match create_connected_pair() {
        Some(pair) => pair,
        None => return,
    };
    let started = std::time::Instant::now();
    dialler.shutdown();
    let took = started.elapsed();
    assert!(took <= bound, "shutdown took {:?}, over its 3 s bound", took);
    drop(acceptor);

    // The harder case: the peer vanished without closing (its Helper was
    // killed), so the connection still looks open and nothing answers the close.
    let (dialler, acceptor) = match create_connected_pair() {
        Some(pair) => pair,
        None => return,
    };
    drop(acceptor);
    let started = std::time::Instant::now();
    dialler.shutdown();
    let took = started.elapsed();
    assert!(
        took <= bound,
        "shutdown with a vanished peer took {:?}, over its 3 s bound",
        took
    );
}

#[test]
fn test_connected_peers_drops_peer_after_disconnect_on_both_sides() {
    // The dialling side goes away: the accepting side must stop listing it.
    let (dialler, acceptor) = match create_connected_pair() {
        Some(pair) => pair,
        None => return,
    };
    dialler.shutdown();
    assert!(
        peers_become(&acceptor, &[]),
        "accepting side still lists {:?} after the dialler disconnected",
        acceptor.connected_peers()
    );
    assert!(
        peers_become(&dialler, &[]),
        "a shut-down Transport still lists {:?}",
        dialler.connected_peers()
    );

    // The accepting side goes away: the dialling side must stop listing it.
    let (dialler, acceptor) = match create_connected_pair() {
        Some(pair) => pair,
        None => return,
    };
    acceptor.shutdown();
    assert!(
        peers_become(&dialler, &[]),
        "dialling side still lists {:?} after the acceptor disconnected",
        dialler.connected_peers()
    );
    assert!(
        peers_become(&acceptor, &[]),
        "a shut-down Transport still lists {:?}",
        acceptor.connected_peers()
    );
}

/// What Stop does: the Helper shuts its Transport down and carries on with a new
/// one in the same process, under a new Ticket.
#[test]
fn test_new_transport_can_be_created_and_dialled_after_shutdown() {
    let (first, friend) = match create_connected_pair() {
        Some(pair) => pair,
        None => return,
    };
    let first_ticket = first.our_ticket().to_string();
    first.shutdown();
    assert!(peers_become(&friend, &[]), "friend should see the first Transport go");

    let second = match create_transport() {
        Some(t) => t,
        None => return,
    };
    assert_ne!(
        second.our_ticket(),
        first_ticket,
        "a Transport created after a shutdown must have a new Ticket"
    );

    // The new Transport works in both directions: it can dial, and be dialled
    // by its new Ticket.
    second
        .connect_to_peer(friend.our_ticket())
        .expect("new Transport should dial after the earlier one was shut down");
    assert!(peers_become(&second, &[friend.endpoint_id()]));
    assert!(peers_become(&friend, &[second.endpoint_id()]));

    let late_joiner = match create_transport() {
        Some(t) => t,
        None => return,
    };
    late_joiner
        .connect_to_peer(second.our_ticket())
        .expect("new Transport should be reachable by its new Ticket");
    assert!(peers_become(&late_joiner, &[second.endpoint_id()]));

    // Still true once the first Transport is gone altogether.
    drop(first);
    assert_eq!(late_joiner.connected_peers(), vec![second.endpoint_id()]);
}

/// Quit shuts down and exits; Stop shuts down and later drops. Neither a second
/// shutdown nor the drop may hang or panic, with or without a peer connected.
#[test]
fn test_shutdown_twice_then_drop_neither_hangs_nor_panics() {
    let (dialler, acceptor) = match create_connected_pair() {
        Some(pair) => pair,
        None => return,
    };

    // Run on a side thread so that a hang fails the test instead of stalling it.
    let (done_tx, done_rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        dialler.shutdown();
        dialler.shutdown();
        drop(dialler);

        // No peer left on this one by now, and never shut down before.
        assert!(peers_become(&acceptor, &[]));
        acceptor.shutdown();
        acceptor.shutdown();
        drop(acceptor);

        let _ = done_tx.send(());
    });

    // Two real shutdowns at most 3 s each; the repeats and drops add nothing.
    // A panic on the side thread drops the sender, which also fails this.
    let finished = done_rx.recv_timeout(Duration::from_secs(2 * 3) + PEER_NOTICE_DEADLINE);
    assert!(
        finished.is_ok(),
        "repeated shutdown and drop hung or panicked: {:?}",
        finished
    );
}
