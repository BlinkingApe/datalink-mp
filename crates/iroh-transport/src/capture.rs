//! Dev-only traffic capture, for measuring what crosses the wire in a Turn sync.
//!
//! Off unless `DATALINK_CAPTURE=<dir>` is set. Then each Helper process writes
//! one JSON Lines file, `<dir>/capture-<unix secs>-<pid>.jsonl`, holding every
//! `GameMessage` in both directions (with its raw payload), the moments the
//! Helper wrote it to the stream or the DLL drained it for the game, each
//! connection's path changes, and the selected path's stats once a second.
//! `docs/traffic-capture.md` describes the format.
//!
//! Nothing here changes what the Helper sends or delivers. Lines go to a writer
//! thread, so the message path only formats a line and hands it over.

use crate::protocol::Message;
use dp_types::DPID;
use iroh::endpoint::{Connection, PathEvent};
use iroh::{EndpointId, TransportAddr};
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

/// How long the writer waits for a line before flushing what it has.
const IDLE_FLUSH: Duration = Duration::from_millis(250);

static CAPTURE: OnceLock<Option<Capture>> = OnceLock::new();

/// An open capture file.
pub struct Capture {
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
        peer: Option<&EndpointId>,
        from: DPID,
        to: DPID,
        guaranteed: bool,
        data: &[u8],
    ) -> u64 {
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
        self.write(line);
        id
    }

    /// Record a later moment in a game message's life.
    fn mark(&self, ev: &str, id: u64) {
        self.write(json!({ "ev": ev, "id": id, "t_us": self.t_us() }));
    }

    fn path_event(&self, peer: &EndpointId, event: &PathEvent) {
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

    fn path_stats(&self, peer: &EndpointId, connection: &Connection) {
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

/// The writer thread: lines in arrival order, flushed whenever traffic pauses.
fn write_lines(mut out: BufWriter<File>, rx: mpsc::Receiver<String>) {
    loop {
        match rx.recv_timeout(IDLE_FLUSH) {
            Ok(line) => {
                if writeln!(out, "{}", line).is_err() {
                    error!("traffic capture: write failed, capture stopped");
                    return;
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                let _ = out.flush();
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                let _ = out.flush();
                return;
            }
        }
    }
}

/// Short hex form of an endpoint id, as the Helper's log lines show it.
fn peer_label(endpoint_id: &EndpointId) -> String {
    let b = endpoint_id.as_bytes();
    format!("{:08x}", u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
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

/// The JACKAL fields at the front of a game payload, as `terranx.exe` lays them
/// out: an 8-byte JACKAL header, then the game's own message.
#[derive(Debug, PartialEq, Eq)]
pub struct Jackal {
    /// Wire offset 0: 4 = reliable data, 2 = ack.
    pub kind: u16,
    /// Wire offset 4: the sender's sequence number for a reliable send.
    pub seq: u32,
    /// Wire offset 8: the game's message type, for data only.
    pub game_type: Option<u16>,
    /// Wire offset 12: the sender's faction, for data only.
    pub faction: Option<u32>,
}

impl Jackal {
    /// Decode the header, or `None` for a payload shorter than it.
    pub fn decode(data: &[u8]) -> Option<Self> {
        let u16_at = |o: usize| data.get(o..o + 2).map(|b| u16::from_le_bytes([b[0], b[1]]));
        let u32_at =
            |o: usize| data.get(o..o + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]));
        let kind = u16_at(0)?;
        let seq = u32_at(4)?;
        // An ack's bytes after the header are never written, so only data has a
        // game message to read.
        let is_data = kind & 0x4 != 0;
        Some(Jackal {
            kind,
            seq,
            game_type: if is_data { u16_at(8) } else { None },
            faction: if is_data { u32_at(12) } else { None },
        })
    }
}

/// Open the capture if `DATALINK_CAPTURE` is set. Called when a Transport
/// starts; later calls do nothing.
pub fn init() {
    capture();
}

fn capture() -> Option<&'static Capture> {
    CAPTURE
        .get_or_init(|| {
            let dir = std::env::var_os(CAPTURE_ENV)?;
            match Capture::open(Path::new(&dir)) {
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

/// Whether a capture is running.
pub fn enabled() -> bool {
    capture().is_some()
}

/// A message about to be queued to `peer`. Returns the id for [`written`] if
/// it's a game message and a capture is running.
pub fn outbound(peer: &EndpointId, msg: &Message) -> Option<u64> {
    let c = capture()?;
    match msg {
        Message::GameMessage { from, to, flags, data } => Some(c.game_message(
            "out",
            Some(peer),
            *from,
            *to,
            flags & dp_types::DPSEND_GUARANTEED != 0,
            data,
        )),
        _ => None,
    }
}

/// A game message the host's game sent to its own server player: it never
/// leaves the Helper, and comes back through the game's inbox.
pub fn loopback(from: DPID, to: DPID, guaranteed: bool, data: &[u8]) -> Option<u64> {
    Some(capture()?.game_message("loop", None, from, to, guaranteed, data))
}

/// A game message that arrived from `peer`.
pub fn inbound(peer: &EndpointId, from: DPID, to: DPID, guaranteed: bool, data: &[u8]) -> Option<u64> {
    Some(capture()?.game_message("in", Some(peer), from, to, guaranteed, data))
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
pub fn spawn_path_watcher(connection: Connection, peer: EndpointId) {
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
    fn decodes_a_data_message() {
        // kind 4, seq 7, then type 0x2303 from faction 3.
        let mut data = vec![4, 0, 0, 0, 7, 0, 0, 0, 0x03, 0x23, 0, 0, 3, 0, 0, 0];
        data.extend_from_slice(&[0; 28]);
        assert_eq!(
            Jackal::decode(&data),
            Some(Jackal { kind: 4, seq: 7, game_type: Some(0x2303), faction: Some(3) })
        );
    }

    #[test]
    fn an_ack_has_no_game_message() {
        let data = [2, 0, 0, 0, 7, 0, 0, 0, 0xaa, 0xbb, 0xcc, 0xdd];
        assert_eq!(
            Jackal::decode(&data),
            Some(Jackal { kind: 2, seq: 7, game_type: None, faction: None })
        );
    }

    #[test]
    fn a_short_payload_has_no_header() {
        assert_eq!(Jackal::decode(&[4, 0, 0, 0, 7]), None);
    }

    #[test]
    fn writes_messages_and_marks_as_lines() {
        let dir = std::env::temp_dir().join(format!("datalink-capture-test-{}", std::process::id()));
        let (capture, path) = Capture::open(&dir).unwrap();
        let peer = iroh::SecretKey::generate().public();
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
