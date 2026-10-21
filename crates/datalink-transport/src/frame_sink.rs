//! The frame sink: the seam under the per-friend ordered writer.
//!
//! The writer's rule lives in [`run_ordered_writer`]: open the stream lazily on
//! the first frame, on a failed write reopen once, replay the frames the
//! friend pipeline says are in flight (the ones already written, in order),
//! then retry that frame, and if the reopen, a replay or the retry fails close
//! the connection and write nothing more.
//! Production uses [`IrohFrameSink`]; tests use [`MemoryFrameSink`].

use crate::connection::STREAM_MAGIC;
use crate::{TransportError, TransportResult};
use iroh::endpoint::{Connection, SendStream};
use std::future::Future;
use tokio::sync::mpsc;
use tracing::{debug, error, warn};

/// Where the ordered writer puts its bytes.
pub trait FrameSink: Send + 'static {
    /// Open (or reopen) the ordered stream and write the magic + Peer protocol
    /// version preamble. Replaces any previously open stream.
    fn open(&mut self) -> impl Future<Output = TransportResult<()>> + Send;
    /// Write one length-prefixed frame on the open stream.
    fn write_frame(&mut self, data: &[u8]) -> impl Future<Output = TransportResult<()>> + Send;
    /// Give up: close the connection.
    fn close(&mut self, reason: &'static [u8]);
    /// The outbox ended normally: finish the stream gracefully.
    fn finish(&mut self);
}

/// How many recently written frames the writer remembers, to tell which
/// in-flight frames it has already put on a stream. Far above the in-flight cap.
const RECENT_FRAMES: usize = 1024;

fn frame_hash(data: &[u8]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    data.hash(&mut h);
    h.finish()
}

/// The writer: drains `outbox_rx` into `sink` in queue order. `in_flight`
/// gives the friend pipeline's in-flight frames, asked only when a stream has
/// to be reopened: those the writer already wrote are replayed on the new
/// stream, in order, before the frame being retried.
pub async fn run_ordered_writer<S: FrameSink>(
    mut sink: S,
    mut outbox_rx: mpsc::UnboundedReceiver<(Vec<u8>, Option<u64>)>,
    peer: [u8; 32],
    peer_label: String,
    in_flight: impl Fn() -> Vec<Vec<u8>> + Send + 'static,
) {
    let mut opened = false;
    let mut recent: std::collections::VecDeque<u64> = std::collections::VecDeque::new();
    while let Some((data, capture_id)) = outbox_rx.recv().await {
        if !opened {
            if let Err(e) = sink.open().await {
                error!(peer = %peer_label, error = %e, "failed to open ordered stream; dropping connection");
                sink.close(b"stream open failed");
                return;
            }
            opened = true;
        }
        if let Err(e) = sink.write_frame(&data).await {
            warn!(peer = %peer_label, error = %e, "ordered stream write failed; reopening once");
            if let Err(e2) = sink.open().await {
                error!(peer = %peer_label, error = %e2, "ordered stream reopen failed; dropping connection");
                sink.close(b"stream reopen failed");
                return;
            }
            let retry = frame_hash(&data);
            let mut replayed = 0;
            for old in in_flight() {
                let h = frame_hash(&old);
                if h == retry || !recent.contains(&h) {
                    continue;
                }
                if let Err(e2) = sink.write_frame(&old).await {
                    error!(peer = %peer_label, error = %e2, "ordered stream replay failed; dropping connection");
                    sink.close(b"stream replay failed");
                    return;
                }
                replayed += 1;
            }
            if replayed > 0 {
                crate::capture::replay(&peer, replayed);
            }
            if let Err(e2) = sink.write_frame(&data).await {
                error!(peer = %peer_label, error = %e2, "ordered stream retry failed; dropping connection");
                sink.close(b"stream write failed");
                return;
            }
        }
        if recent.len() == RECENT_FRAMES {
            recent.pop_front();
        }
        recent.push_back(frame_hash(&data));
        if let Some(id) = capture_id {
            crate::capture::written(id);
        }
    }
    sink.finish();
    debug!(peer = %peer_label, "ordered writer task exiting");
}

/// Production adapter: a uni-directional QUIC stream on an iroh connection.
pub struct IrohFrameSink {
    connection: Connection,
    peer_protocol_version: u16,
    stream: Option<SendStream>,
}

impl IrohFrameSink {
    pub fn new(connection: Connection, peer_protocol_version: u16) -> Self {
        Self { connection, peer_protocol_version, stream: None }
    }
}

impl FrameSink for IrohFrameSink {
    async fn open(&mut self) -> TransportResult<()> {
        // A reopen ends the old stream now, so the friend's reader for it
        // sees the end at once and the new stream's reader is not kept
        // waiting (see `stream_turns`).
        self.finish();
        let mut s = self.connection.open_uni().await?;
        let mut preamble = [0u8; 6];
        preamble[..4].copy_from_slice(STREAM_MAGIC);
        preamble[4..].copy_from_slice(&self.peer_protocol_version.to_le_bytes());
        s.write_all(&preamble).await?;
        self.stream = Some(s);
        Ok(())
    }

    async fn write_frame(&mut self, data: &[u8]) -> TransportResult<()> {
        let s = self.stream.as_mut().ok_or(TransportError::NotConnected)?;
        s.write_all(&(data.len() as u32).to_le_bytes()).await?;
        s.write_all(data).await?;
        Ok(())
    }

    fn close(&mut self, reason: &'static [u8]) {
        self.connection.close(1u32.into(), reason);
    }

    fn finish(&mut self) {
        if let Some(mut s) = self.stream.take() {
            let _ = s.finish();
        }
    }
}

/// One thing the in-memory sink saw.
#[cfg(test)]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SinkEvent {
    Preamble,
    Frame(Vec<u8>),
    Closed,
}

/// Test adapter: records preambles and frames; fails a chosen open or write.
/// Opens and writes are counted from 0 over the sink's whole life, failed
/// attempts included.
#[cfg(test)]
#[derive(Clone, Default)]
pub struct MemoryFrameSink {
    pub events: std::sync::Arc<parking_lot::Mutex<Vec<SinkEvent>>>,
    fail_opens: std::collections::HashSet<usize>,
    fail_writes: std::collections::HashSet<usize>,
    opens: usize,
    writes: usize,
}

#[cfg(test)]
impl MemoryFrameSink {
    pub fn failing(opens: &[usize], writes: &[usize]) -> Self {
        Self {
            fail_opens: opens.iter().copied().collect(),
            fail_writes: writes.iter().copied().collect(),
            ..Default::default()
        }
    }
}

#[cfg(test)]
impl FrameSink for MemoryFrameSink {
    async fn open(&mut self) -> TransportResult<()> {
        let n = self.opens;
        self.opens += 1;
        if self.fail_opens.contains(&n) {
            return Err(TransportError::NotConnected);
        }
        self.events.lock().push(SinkEvent::Preamble);
        Ok(())
    }

    async fn write_frame(&mut self, data: &[u8]) -> TransportResult<()> {
        let n = self.writes;
        self.writes += 1;
        if self.fail_writes.contains(&n) {
            return Err(TransportError::NotConnected);
        }
        self.events.lock().push(SinkEvent::Frame(data.to_vec()));
        Ok(())
    }

    fn close(&mut self, _reason: &'static [u8]) {
        self.events.lock().push(SinkEvent::Closed);
    }

    fn finish(&mut self) {}
}

#[cfg(test)]
mod tests {
    use super::SinkEvent::*;
    use super::*;

    async fn run(sink: MemoryFrameSink, frames: &[&[u8]]) -> Vec<SinkEvent> {
        run_with_in_flight(sink, frames, &[]).await
    }

    /// `in_flight` is what the friend pipeline would hand the writer on a reopen.
    async fn run_with_in_flight(
        sink: MemoryFrameSink,
        frames: &[&[u8]],
        in_flight: &[&[u8]],
    ) -> Vec<SinkEvent> {
        let in_flight: Vec<Vec<u8>> = in_flight.iter().map(|f| f.to_vec()).collect();
        let events = sink.events.clone();
        let (tx, rx) = mpsc::unbounded_channel();
        for f in frames {
            tx.send((f.to_vec(), None)).unwrap();
        }
        drop(tx);
        run_ordered_writer(sink, rx, [0; 32], "test".into(), move || in_flight.clone()).await;
        let v = events.lock().clone();
        v
    }

    fn f(b: &[u8]) -> SinkEvent {
        Frame(b.to_vec())
    }

    #[tokio::test]
    async fn frames_arrive_in_order_after_one_preamble() {
        let ev = run(MemoryFrameSink::default(), &[b"a", b"b", b"c"]).await;
        assert_eq!(ev, vec![Preamble, f(b"a"), f(b"b"), f(b"c")]);
    }

    #[tokio::test]
    async fn failed_write_reopens_and_retries_same_frame() {
        // write #1 (frame "b") fails
        let ev = run(MemoryFrameSink::failing(&[], &[1]), &[b"a", b"b", b"c"]).await;
        assert_eq!(ev, vec![Preamble, f(b"a"), Preamble, f(b"b"), f(b"c")]);
    }

    #[tokio::test]
    async fn failed_reopen_closes_and_writes_nothing_more() {
        // write #1 fails, then open #1 (the reopen) fails
        let ev = run(MemoryFrameSink::failing(&[1], &[1]), &[b"a", b"b", b"c"]).await;
        assert_eq!(ev, vec![Preamble, f(b"a"), Closed]);
    }

    #[tokio::test]
    async fn failed_retry_closes_and_writes_nothing_more() {
        // write #1 fails and so does the retry (write #2)
        let ev = run(MemoryFrameSink::failing(&[], &[1, 2]), &[b"a", b"b", b"c"]).await;
        assert_eq!(ev, vec![Preamble, f(b"a"), Preamble, Closed]);
    }

    #[tokio::test]
    async fn failed_first_open_closes() {
        let ev = run(MemoryFrameSink::failing(&[0], &[]), &[b"a"]).await;
        assert_eq!(ev, vec![Closed]);
    }

    /// What the friend's JACKAL delivers: each frame once, first copy kept.
    fn delivered(ev: &[SinkEvent]) -> Vec<SinkEvent> {
        let mut seen = std::collections::HashSet::new();
        ev.iter().filter(|e| matches!(e, Frame(_)) && seen.insert((*e).clone())).cloned().collect()
    }

    #[tokio::test]
    async fn a_reset_stream_replays_in_flight_frames_in_order_before_the_retried_frame() {
        // write #2 (frame "c") fails; "a" and "b" are in flight, "d" is queued
        // and in flight but not written yet, so it is not replayed.
        let ev = run_with_in_flight(
            MemoryFrameSink::failing(&[], &[2]),
            &[b"a", b"b", b"c", b"d"],
            &[b"a", b"b", b"d"],
        )
        .await;
        assert_eq!(
            ev,
            vec![Preamble, f(b"a"), f(b"b"), Preamble, f(b"a"), f(b"b"), f(b"c"), f(b"d")]
        );
        assert_eq!(delivered(&ev), vec![f(b"a"), f(b"b"), f(b"c"), f(b"d")]);
    }

    #[tokio::test]
    async fn the_retried_frame_is_not_replayed_twice_even_if_in_flight() {
        let ev = run_with_in_flight(
            MemoryFrameSink::failing(&[], &[1]),
            &[b"a", b"b"],
            &[b"a", b"b"],
        )
        .await;
        assert_eq!(ev, vec![Preamble, f(b"a"), Preamble, f(b"a"), f(b"b")]);
    }

    #[tokio::test]
    async fn a_failed_replay_write_closes_the_connection() {
        // write #1 fails, then the replay of "a" (write #2) fails
        let ev = run_with_in_flight(
            MemoryFrameSink::failing(&[], &[1, 2]),
            &[b"a", b"b"],
            &[b"a"],
        )
        .await;
        assert_eq!(ev, vec![Preamble, f(b"a"), Preamble, Closed]);
    }
}
