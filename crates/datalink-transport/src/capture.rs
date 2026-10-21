//! Dev-only traffic capture, for measuring what crosses the wire in a Turn sync.
//!
//! Off unless `DATALINK_CAPTURE=<dir>` is set. Then each Helper process writes
//! one JSON Lines file, `<dir>/capture-<unix secs>-<pid>.jsonl`, holding every
//! `GameMessage` in both directions (with its raw payload), the moments the
//! Helper wrote it to the stream or the DLL drained it for the game, each
//! connection's path changes, and the selected path's stats once a second.
//! Around those it records what explains a session's life: the Peer protocol's
//! other messages, the session settings the game sets, the system messages the
//! game is handed, failed sends, and lost connections.
//! `docs/contributors/traffic-capture.md` describes the format.
//!
//! Nothing here changes what the Helper sends or delivers. Lines go to a writer
//! thread, so the message path only formats a line and hands it over.

use crate::jackal::Jackal;
use crate::protocol::Message;
use dp_types::{SessionDesc, DPID};
use iroh::endpoint::{Connection, PathEvent};
use iroh::TransportAddr;
use serde_json::{json, Value};
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{mpsc, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tracing::{error, info};

/// Environment variable naming the directory captures are written to.
pub const CAPTURE_ENV: &str = "DATALINK_CAPTURE";

/// Version of the capture's line format. Bump it when a field changes meaning.
pub const CAPTURE_FORMAT_VERSION: u32 = 1;

/// How often each connection's selected path is sampled.
const STATS_INTERVAL: Duration = Duration::from_secs(1);

static CAPTURE: OnceLock<Option<Capture>> = OnceLock::new();

/// An open capture file.
pub struct Capture {
    path: PathBuf,
    start: Instant,
    next_id: AtomicU64,
    lines: mpsc::Sender<String>,
}

impl Capture {
    /// Open a new capture file in `dir` and start its writer thread.
    pub fn open(dir: &Path) -> std::io::Result<(Self, PathBuf)> {
        std::fs::create_dir_all(dir)?;
        let unix = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
        let path = dir.join(format!("capture-{}-{}.jsonl", unix.as_secs(), std::process::id()));
        let file = File::create(&path)?;

        let (lines, rx) = mpsc::channel::<String>();
        std::thread::Builder::new()
            .name("datalink-capture".into())
            .spawn(move || write_lines(BufWriter::new(file), rx))?;

        let capture = Capture {
            path: path.clone(),
            start: Instant::now(),
            next_id: AtomicU64::new(1),
            lines,
        };
        capture.write(json!({
            "ev": "start",
            "t_us": 0,
            "format": CAPTURE_FORMAT_VERSION,
            "unix_ms": unix.as_millis() as u64,
            "pid": std::process::id(),
        }));
        Ok((capture, path))
    }

    fn t_us(&self) -> u64 {
        self.start.elapsed().as_micros() as u64
    }

    fn next_id(&self) -> u64 {
        self.next_id.fetch_add(1, Ordering::Relaxed)
    }

    fn write(&self, line: Value) {
        // The writer thread only goes away with the process.
        let _ = self.lines.send(line.to_string());
    }

    /// Record a game message and return the id its later events refer to.
    fn game_message(
        &self,
        ev: &str,
        peer: Option<&[u8; 32]>,
        from: DPID,
        to: DPID,
        guaranteed: bool,
        data: &[u8],
    ) -> u64 {
        let (id, line) = self.message_line(ev, peer, from, to, guaranteed, data);
        self.write(line);
        id
    }

    /// A game message's line, with a fresh id.
    fn message_line(
        &self,
        ev: &str,
        peer: Option<&[u8; 32]>,
        from: DPID,
        to: DPID,
        guaranteed: bool,
        data: &[u8],
    ) -> (u64, Value) {
        let id = self.next_id();
        let mut line = json!({
            "ev": ev,
            "id": id,
            "t_us": self.t_us(),
            "peer": peer.map(peer_label),
            "from": from,
            "to": to,
            "guaranteed": guaranteed,
            "size": data.len(),
        });
        if let Some(j) = Jackal::decode(data) {
            line["kind"] = json!(j.kind);
            line["seq"] = json!(j.seq);
            if let Some(t) = j.game_type {
                line["type"] = json!(format!("{:#06x}", t));
            }
            if let Some(f) = j.faction {
                line["faction"] = json!(f);
            }
        }
        line["hex"] = json!(data_encoding::HEXLOWER.encode(data));
        (id, line)
    }

    /// A Peer protocol message other than a game message, in or out.
    fn control(&self, dir: &str, peer: &[u8; 32], msg: &Message) {
        let mut line = json!({
            "ev": "ctl",
            "t_us": self.t_us(),
            "dir": dir,
            "peer": peer_label(peer),
            "msg": msg.type_name(),
        });
        match msg {
            Message::SessionDescUpdate { session } => add_session(&mut line, session),
            Message::PlayerLeft { player_id, .. } => line["player"] = json!(player_id),
            Message::PlayerJoined { player } => line["player"] = json!(player.player_id),
            Message::PlayerNameUpdate { player_id, .. } => line["player"] = json!(player_id),
            Message::PlayerDataUpdate { player_id, data, .. } => {
                line["player"] = json!(player_id);
                line["size"] = json!(data.len());
            }
            Message::Hello { player_id, .. } => line["player"] = json!(player_id),
            Message::HostMigration { new_host_id } => line["player"] = json!(new_host_id),
            _ => {}
        }
        self.write(line);
    }

    /// An Early ack was given to the game for the `out` message `id`, after
    /// being withheld for `withheld_us` (0 when it was made at once).
    fn early_ack(&self, peer: &[u8; 32], seq: u32, id: Option<u64>, withheld_us: u64) {
        self.write(json!({
            "ev": "early_ack", "t_us": self.t_us(), "peer": peer_label(peer),
            "seq": seq, "id": id, "withheld_us": withheld_us,
        }));
    }

    /// The real ack of an Early-acked message was swallowed, `rtt_us` after
    /// the message was queued.
    fn real_ack(&self, peer: &[u8; 32], seq: u32, rtt_us: u64) {
        self.write(json!({
            "ev": "real_ack", "t_us": self.t_us(), "peer": peer_label(peer),
            "seq": seq, "rtt_us": rtt_us,
        }));
    }

    /// A resend of a message this connection already carried was dropped.
    fn resend_dropped(&self, peer: &[u8; 32], seq: u32) {
        self.write(json!({
            "ev": "resend_dropped", "t_us": self.t_us(), "peer": peer_label(peer), "seq": seq,
        }));
    }

    /// `count` in-flight frames were replayed to `peer` on a reopened stream.
    fn replay(&self, peer: &[u8; 32], count: usize) {
        self.write(json!({
            "ev": "replay", "t_us": self.t_us(), "peer": peer_label(peer), "count": count,
        }));
    }

    /// Whether Early acks are on for `peer` this Game session, and why.
    fn early_acks_state(&self, peer: &[u8; 32], on: bool, why: &str) {
        self.write(json!({
            "ev": "early_acks", "t_us": self.t_us(), "peer": peer_label(peer),
            "on": on, "why": why,
        }));
    }

    /// Record a later moment in a game message's life.
    fn mark(&self, ev: &str, id: u64) {
        self.write(json!({ "ev": ev, "id": id, "t_us": self.t_us() }));
    }

    fn path_event(&self, peer: &[u8; 32], event: &PathEvent) {
        let mut line = json!({ "ev": "path", "t_us": self.t_us(), "peer": peer_label(peer) });
        match event {
            PathEvent::Opened { id, remote_addr, .. } => {
                line["change"] = json!("opened");
                line["path"] = json!(id.to_string());
                add_route(&mut line, remote_addr);
            }
            PathEvent::Closed { id, remote_addr, last_stats, .. } => {
                line["change"] = json!("closed");
                line["path"] = json!(id.to_string());
                add_route(&mut line, remote_addr);
                line["rtt_us"] = json!(last_stats.rtt.as_micros() as u64);
                line["lost_packets"] = json!(last_stats.lost_packets);
            }
            PathEvent::Selected { id, remote_addr, .. } => {
                line["change"] = json!("selected");
                line["path"] = json!(id.to_string());
                add_route(&mut line, remote_addr);
            }
            PathEvent::Lagged { missed, .. } => {
                line["change"] = json!("lagged");
                line["missed"] = json!(missed);
            }
            _ => line["change"] = json!("other"),
        }
        self.write(line);
    }

    fn path_stats(&self, peer: &[u8; 32], connection: &Connection) {
        let mut line = json!({ "ev": "stats", "t_us": self.t_us(), "peer": peer_label(peer) });
        let paths = connection.paths();
        line["open_paths"] = json!(paths.iter().count());
        match paths.iter().find(|p| p.is_selected()) {
            Some(selected) => {
                add_route(&mut line, selected.remote_addr());
                line["path"] = json!(selected.id().to_string());
                let s = selected.stats();
                line["rtt_us"] = json!(s.rtt.as_micros() as u64);
                line["cwnd"] = json!(s.cwnd);
                line["tx_datagrams"] = json!(s.udp_tx.datagrams);
                line["tx_bytes"] = json!(s.udp_tx.bytes);
                line["rx_datagrams"] = json!(s.udp_rx.datagrams);
                line["rx_bytes"] = json!(s.udp_rx.bytes);
                line["lost_packets"] = json!(s.lost_packets);
                line["lost_bytes"] = json!(s.lost_bytes);
                line["congestion_events"] = json!(s.congestion_events);
                line["mtu"] = json!(s.current_mtu);
            }
            None => line["route"] = json!("none"),
        }
        let c = connection.stats();
        line["conn_tx_bytes"] = json!(c.udp_tx.bytes);
        line["conn_rx_bytes"] = json!(c.udp_rx.bytes);
        line["conn_lost_packets"] = json!(c.lost_packets);
        self.write(line);
    }
}

/// The writer thread: lines in arrival order, flushed as soon as none are
/// waiting, so a Helper that's closed or killed loses as little as possible.
fn write_lines(mut out: BufWriter<File>, rx: mpsc::Receiver<String>) {
    while let Ok(first) = rx.recv() {
        for line in std::iter::once(first).chain(rx.try_iter()) {
            if writeln!(out, "{}", line).is_err() {
                error!("traffic capture: write failed, capture stopped");
                return;
            }
        }
        if out.flush().is_err() {
            error!("traffic capture: write failed, capture stopped");
            return;
        }
    }
}

/// Short hex form of an endpoint id, as the Helper's log lines show it.
fn peer_label(b: &[u8; 32]) -> String {
    format!("{:08x}", u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
}

/// Add a session's settings to a line: the flags that matter for joining, the
/// player counts and the name.
fn add_session(line: &mut Value, desc: &SessionDesc) {
    line["flags"] = json!(format!("{:#010x}", desc.flags));
    line["new_players_disabled"] =
        json!(desc.flags & dp_types::flags::DPSESSION_NEWPLAYERSDISABLED != 0);
    line["join_disabled"] = json!(desc.is_in_progress());
    line["current_players"] = json!(desc.current_players);
    line["max_players"] = json!(desc.max_players);
    line["name"] = json!(desc.session_name);
}

/// The name of a DirectPlay system message type.
fn system_message_name(dw_type: u32) -> &'static str {
    use dp_types::structs::sysmsg::*;
    match dw_type {
        DPSYS_CREATEPLAYERORGROUP => "CREATEPLAYERORGROUP",
        DPSYS_DESTROYPLAYERORGROUP => "DESTROYPLAYERORGROUP",
        DPSYS_SESSIONLOST => "SESSIONLOST",
        DPSYS_HOST => "HOST",
        DPSYS_SETPLAYERORGROUPDATA => "SETPLAYERORGROUPDATA",
        DPSYS_SETPLAYERORGROUPNAME => "SETPLAYERORGROUPNAME",
        DPSYS_SETSESSIONDESC => "SETSESSIONDESC",
        DPSYS_STARTSESSION => "STARTSESSION",
        DPSYS_CHAT => "CHAT",
        _ => "other",
    }
}

/// Add a path's route ("direct" or "relayed") and its address to a line.
fn add_route(line: &mut Value, addr: &TransportAddr) {
    match addr {
        TransportAddr::Ip(ip) => {
            line["route"] = json!("direct");
            line["addr"] = json!(ip.to_string());
        }
        TransportAddr::Relay(url) => {
            line["route"] = json!("relayed");
            line["addr"] = json!(url.to_string());
        }
        other => {
            line["route"] = json!("other");
            line["addr"] = json!(format!("{:?}", other));
        }
    }
}

/// Open the capture if `DATALINK_CAPTURE` is set. Called when a Transport
/// starts; later calls do nothing.
pub fn init() {
    capture();
}

/// The directory a capture goes in, given the variable's value: none when it's
/// unset or empty.
fn capture_dir(value: Option<std::ffi::OsString>) -> Option<PathBuf> {
    value.filter(|v| !v.is_empty()).map(PathBuf::from)
}

fn capture() -> Option<&'static Capture> {
    CAPTURE
        .get_or_init(|| {
            let dir = capture_dir(std::env::var_os(CAPTURE_ENV))?;
            match Capture::open(&dir) {
                Ok((capture, path)) => {
                    info!(path = %path.display(), "traffic capture on");
                    Some(capture)
                }
                Err(e) => {
                    error!(dir = ?dir, error = %e, "traffic capture: can't open a file, capture off");
                    None
                }
            }
        })
        .as_ref()
}

/// The running capture's file, if there is one.
pub fn path() -> Option<&'static Path> {
    capture().map(|c| c.path.as_path())
}

/// Whether a capture is running.
pub fn enabled() -> bool {
    capture().is_some()
}

/// A game message about to be queued to `peer`. Returns the id for [`written`]
/// if a capture is running.
pub fn outbound_game(peer: &[u8; 32], from: DPID, to: DPID, guaranteed: bool, data: &[u8]) -> Option<u64> {
    Some(capture()?.game_message("out", Some(peer), from, to, guaranteed, data))
}

/// A Peer protocol message other than a game message about to be queued to
/// `peer`. Outbound game messages are recorded by [`outbound_game`], from the
/// friend's pipeline.
pub fn outbound_control(peer: &[u8; 32], msg: &Message) {
    if let Some(c) = capture() {
        c.control("out", peer, msg);
    }
}

/// A Peer protocol message other than a game message arrived from `peer`.
pub fn inbound_control(peer: &[u8; 32], msg: &Message) {
    if let Some(c) = capture() {
        c.control("in", peer, msg);
    }
}

/// The game set its session's settings. Only the host's are applied and passed on.
pub fn session_desc_set(desc: &SessionDesc, is_host: bool) {
    if let Some(c) = capture() {
        let mut line = json!({ "ev": "session_desc", "t_us": c.t_us(), "host": is_host });
        add_session(&mut line, desc);
        c.write(line);
    }
}

/// A game message the Helper couldn't queue to anyone.
pub fn send_failed(from: DPID, to: DPID, guaranteed: bool, data: &[u8], error: &str) {
    if let Some(c) = capture() {
        let (_, mut line) = c.message_line("send_failed", None, from, to, guaranteed, data);
        line["error"] = json!(error);
        c.write(line);
    }
}

/// The DLL drained a DirectPlay system message for the game.
pub fn system_drained(data: &[u8]) {
    let Some(c) = capture() else { return };
    let u32_at = |o: usize| data.get(o..o + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]));
    let mut line = json!({ "ev": "sys", "t_us": c.t_us(), "size": data.len() });
    if let Some(dw_type) = u32_at(0) {
        line["sys"] = json!(system_message_name(dw_type));
        line["sys_type"] = json!(format!("{:#06x}", dw_type));
        use dp_types::structs::sysmsg::*;
        // These start dwType, dwPlayerType, dpId.
        if matches!(
            dw_type,
            DPSYS_CREATEPLAYERORGROUP
                | DPSYS_DESTROYPLAYERORGROUP
                | DPSYS_SETPLAYERORGROUPDATA
                | DPSYS_SETPLAYERORGROUPNAME
        ) {
            line["player"] = json!(u32_at(8));
        }
    }
    line["hex"] = json!(data_encoding::HEXLOWER.encode(data));
    c.write(line);
}

/// The Helper learned a friend's connection was lost.
pub fn connection_lost(peer: &[u8; 32], was_host: bool) {
    if let Some(c) = capture() {
        c.write(json!({ "ev": "lost", "t_us": c.t_us(), "peer": peer_label(peer), "host": was_host }));
    }
}

/// One attempt to re-dial a lost friend.
pub fn reconnect(peer: &[u8; 32], attempt: u32, ok: bool) {
    if let Some(c) = capture() {
        c.write(json!({
            "ev": "reconnect",
            "t_us": c.t_us(),
            "peer": peer_label(peer),
            "attempt": attempt,
            "ok": ok,
        }));
    }
}

/// A game message the host's game sent to its own server player: it never
/// leaves the Helper, and comes back through the game's inbox.
pub fn loopback(from: DPID, to: DPID, guaranteed: bool, data: &[u8]) -> Option<u64> {
    Some(capture()?.game_message("loop", None, from, to, guaranteed, data))
}

/// A game message that arrived from `peer`.
pub fn inbound(peer: &[u8; 32], from: DPID, to: DPID, guaranteed: bool, data: &[u8]) -> Option<u64> {
    Some(capture()?.game_message("in", Some(peer), from, to, guaranteed, data))
}

/// An Early ack was given to the game for the `out` message `id` (its capture
/// id, if a capture was running), `withheld_us` after the message was queued
/// when the cap held it back, else 0.
pub fn early_ack(peer: &[u8; 32], seq: u32, id: Option<u64>, withheld_us: u64) {
    if let Some(c) = capture() {
        c.early_ack(peer, seq, id, withheld_us);
    }
}

/// The real ack of an Early-acked message was swallowed.
pub fn real_ack(peer: &[u8; 32], seq: u32, rtt_us: u64) {
    if let Some(c) = capture() {
        c.real_ack(peer, seq, rtt_us);
    }
}

/// A resend the same connection already carried was dropped.
pub fn resend_dropped(peer: &[u8; 32], seq: u32) {
    if let Some(c) = capture() {
        c.resend_dropped(peer, seq);
    }
}

/// `count` in-flight frames were replayed to `peer` after a stream was reopened.
pub fn replay(peer: &[u8; 32], count: usize) {
    if let Some(c) = capture() {
        c.replay(peer, count);
    }
}

/// Once per Game session and friend: Early acks are `on`, or off for `why`
/// (`switch`, `players` or `connection_lost`).
pub fn early_acks_state(peer: &[u8; 32], on: bool, why: &str) {
    if let Some(c) = capture() {
        c.early_acks_state(peer, on, why);
    }
}

/// An outbound message's frame was written to its peer's stream.
pub fn written(id: u64) {
    if let Some(c) = capture() {
        c.mark("written", id);
    }
}

/// An inbound or loopback message was drained by the DLL for the game.
pub fn drained(id: u64) {
    if let Some(c) = capture() {
        c.mark("drain", id);
    }
}

/// Watch a connection's paths for as long as it lives: every path change, and
/// the selected path's stats each second. Does nothing without a capture.
pub fn spawn_path_watcher(connection: Connection, peer: [u8; 32]) {
    let Some(c) = capture() else { return };
    tokio::spawn(async move {
        use n0_future::StreamExt;
        // Subscribe before the first sample, so no change falls between them.
        let mut events = connection.path_events();
        let mut tick = tokio::time::interval(STATS_INTERVAL);
        loop {
            tokio::select! {
                event = events.next() => match event {
                    Some(event) => c.path_event(&peer, &event),
                    None => break,
                },
                _ = tick.tick() => {
                    if connection.close_reason().is_some() {
                        break;
                    }
                    c.path_stats(&peer, &connection);
                }
            }
        }
        c.write(json!({ "ev": "closed", "t_us": c.t_us(), "peer": peer_label(&peer) }));
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_lines_say_whether_joining_is_closed() {
        let desc = SessionDesc {
            flags: dp_types::flags::DPSESSION_NEWPLAYERSDISABLED,
            max_players: 7,
            current_players: 2,
            session_name: "Planet".into(),
            ..Default::default()
        };
        let mut line = json!({});
        add_session(&mut line, &desc);
        assert_eq!(line["flags"], "0x00000001");
        assert_eq!(line["new_players_disabled"], true);
        assert_eq!(line["join_disabled"], false);
        assert_eq!(line["current_players"], 2);
        assert_eq!(line["max_players"], 7);
    }

    #[test]
    fn names_the_system_messages_the_game_is_handed() {
        assert_eq!(system_message_name(0x0005), "DESTROYPLAYERORGROUP");
        assert_eq!(system_message_name(0x0031), "SESSIONLOST");
        assert_eq!(system_message_name(0x7777), "other");
    }

    #[test]
    fn capture_is_off_unless_the_variable_names_a_directory() {
        use std::ffi::OsString;
        assert_eq!(capture_dir(None), None);
        assert_eq!(capture_dir(Some(OsString::new())), None);
        assert_eq!(capture_dir(Some(OsString::from("caps"))), Some(PathBuf::from("caps")));
    }

    #[test]
    fn early_ack_lines_carry_their_fields() {
        let dir = std::env::temp_dir().join(format!("datalink-capture-early-{}", std::process::id()));
        let (capture, path) = Capture::open(&dir).unwrap();
        let peer = [0xab, 0xcd, 0, 1, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9];
        capture.early_acks_state(&peer, false, "players");
        capture.early_ack(&peer, 7, Some(12), 0);
        capture.early_ack(&peer, 8, None, 1500);
        capture.real_ack(&peer, 7, 61_000);
        capture.resend_dropped(&peer, 7);
        capture.replay(&peer, 3);
        drop(capture);
        let mut text = String::new();
        for _ in 0..50 {
            text = std::fs::read_to_string(&path).unwrap();
            if text.lines().count() == 7 {
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        let l: Vec<Value> = text.lines().map(|l| serde_json::from_str(l).unwrap()).collect();
        assert_eq!(l.len(), 7);
        let p = "abcd0001";
        assert_eq!((l[1]["ev"].as_str(), l[1]["peer"].as_str()), (Some("early_acks"), Some(p)));
        assert_eq!((&l[1]["on"], &l[1]["why"]), (&json!(false), &json!("players")));
        assert_eq!(l[2]["ev"], "early_ack");
        assert_eq!((&l[2]["seq"], &l[2]["id"], &l[2]["withheld_us"]), (&json!(7), &json!(12), &json!(0)));
        assert_eq!((&l[3]["id"], &l[3]["withheld_us"]), (&Value::Null, &json!(1500)));
        assert_eq!((&l[4]["ev"], &l[4]["seq"], &l[4]["rtt_us"]), (&json!("real_ack"), &json!(7), &json!(61_000)));
        assert_eq!((&l[5]["ev"], &l[5]["seq"]), (&json!("resend_dropped"), &json!(7)));
        assert_eq!((&l[6]["ev"], &l[6]["count"]), (&json!("replay"), &json!(3)));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn writes_messages_and_marks_as_lines() {
        let dir = std::env::temp_dir().join(format!("datalink-capture-test-{}", std::process::id()));
        let (capture, path) = Capture::open(&dir).unwrap();
        let peer = *iroh::SecretKey::generate().public().as_bytes();
        let data = [4, 0, 0, 0, 9, 0, 0, 0, 0x01, 0x43, 0, 0, 1, 0, 0, 0];
        let id = capture.game_message("in", Some(&peer), 0x10, 0x20, false, &data);
        capture.mark("drain", id);
        drop(capture);

        // The writer flushes and exits once the capture is dropped.
        let mut text = String::new();
        for _ in 0..50 {
            text = std::fs::read_to_string(&path).unwrap();
            if text.lines().count() == 3 {
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        let lines: Vec<Value> = text.lines().map(|l| serde_json::from_str(l).unwrap()).collect();
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0]["ev"], "start");
        assert_eq!(lines[0]["format"], CAPTURE_FORMAT_VERSION);
        assert_eq!(lines[1]["ev"], "in");
        assert_eq!(lines[1]["id"], id);
        assert_eq!(lines[1]["peer"], peer_label(&peer));
        assert_eq!(lines[1]["from"], 0x10);
        assert_eq!(lines[1]["seq"], 9);
        assert_eq!(lines[1]["type"], "0x4301");
        assert_eq!(lines[1]["faction"], 1);
        assert_eq!(lines[1]["hex"], "04000000090000000143000001000000");
        assert_eq!(lines[2]["ev"], "drain");
        assert_eq!(lines[2]["id"], id);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
