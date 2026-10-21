//! Dev-only fault injection, behind the `dev-faults` cargo feature (never on
//! in a release build). `DATALINK_FAULT` holds `drop:N`, `reset:N`, or both
//! comma-separated:
//! - `drop:N` closes the friend's connection right after the Nth Early ack is
//!   made. Counted by the friend registry, where Early acks are made.
//! - `reset:N` resets the ordered stream: the write after the Nth frame
//!   written on the sink fails once, as a reset stream would, so the writer
//!   reopens and replays. A [`FrameSink`] adapter around the real sink.
//!
//! Each fires once and logs a warning when it does.

use crate::frame_sink::FrameSink;
use crate::TransportResult;
use std::future::Future;
use tracing::warn;

/// The environment variable holding the fault spec.
pub const FAULT_ENV: &str = "DATALINK_FAULT";

/// Which faults to inject, and after how many events.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Faults {
    /// Close the friend's connection after this many Early acks.
    pub drop_after: Option<u32>,
    /// Reset the ordered stream after this many frames written.
    pub reset_after: Option<u32>,
}

impl Faults {
    /// Parse `drop:N`, `reset:N` or both, comma-separated.
    pub fn parse(spec: &str) -> Result<Self, String> {
        let mut faults = Faults::default();
        for part in spec.split(',').map(str::trim).filter(|p| !p.is_empty()) {
            let (kind, n) = part.split_once(':').ok_or(format!("bad fault {part:?}"))?;
            let n: u32 = n.trim().parse().map_err(|_| format!("bad count in {part:?}"))?;
            match kind.trim() {
                "drop" => faults.drop_after = Some(n),
                "reset" => faults.reset_after = Some(n),
                other => return Err(format!("unknown fault {other:?}")),
            }
        }
        Ok(faults)
    }

    /// From `DATALINK_FAULT`. Unset gives no faults; a bad spec is logged and
    /// ignored.
    pub fn from_env() -> Self {
        let Ok(spec) = std::env::var(FAULT_ENV) else { return Self::default() };
        Self::parse(&spec).unwrap_or_else(|e| {
            warn!(error = %e, "{FAULT_ENV} ignored");
            Self::default()
        })
    }
}

/// A [`FrameSink`] that fails one write, once, after `after` frames were
/// written through it.
pub struct ResetFrameSink<S> {
    inner: S,
    after: u32,
    written: u32,
    fired: bool,
}

impl<S> ResetFrameSink<S> {
    pub fn new(inner: S, after: u32) -> Self {
        Self { inner, after, written: 0, fired: false }
    }
}

impl<S: FrameSink> FrameSink for ResetFrameSink<S> {
    fn open(&mut self) -> impl Future<Output = TransportResult<()>> + Send {
        self.inner.open()
    }

    async fn write_frame(&mut self, data: &[u8]) -> TransportResult<()> {
        if !self.fired && self.written >= self.after {
            self.fired = true;
            warn!(after = self.after, "dev-faults: resetting the ordered stream");
            return Err(crate::TransportError::NotConnected);
        }
        self.inner.write_frame(data).await?;
        self.written += 1;
        Ok(())
    }

    fn close(&mut self, reason: &'static [u8]) {
        self.inner.close(reason);
    }

    fn finish(&mut self) {
        self.inner.finish();
    }
}

/// Counts Early acks and says when the `drop:N` fault fires (once, on the Nth).
#[derive(Debug)]
pub struct DropCounter {
    after: u32,
    made: u32,
}

impl DropCounter {
    pub fn new(after: u32) -> Self {
        Self { after, made: 0 }
    }

    /// An Early ack was made. True exactly once, when it is the Nth.
    pub fn early_ack_made(&mut self) -> bool {
        self.made += 1;
        let fire = self.made == self.after;
        if fire {
            warn!(after = self.after, "dev-faults: closing the friend's connection");
        }
        fire
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame_sink::{run_ordered_writer, MemoryFrameSink, SinkEvent::*};
    use tokio::sync::mpsc;

    #[test]
    fn parses_drop_reset_both_and_rejects_junk() {
        assert_eq!(Faults::parse("drop:3"), Ok(Faults { drop_after: Some(3), reset_after: None }));
        assert_eq!(Faults::parse("reset:5"), Ok(Faults { drop_after: None, reset_after: Some(5) }));
        assert_eq!(
            Faults::parse("drop:1, reset:2"),
            Ok(Faults { drop_after: Some(1), reset_after: Some(2) })
        );
        assert_eq!(Faults::parse(""), Ok(Faults::default()));
        assert!(Faults::parse("drop").is_err());
        assert!(Faults::parse("drop:x").is_err());
        assert!(Faults::parse("explode:1").is_err());
    }

    #[test]
    fn drop_fires_once_on_the_nth_early_ack() {
        let mut c = DropCounter::new(2);
        assert_eq!((c.early_ack_made(), c.early_ack_made(), c.early_ack_made()), (false, true, false));
    }

    #[tokio::test]
    async fn reset_fails_the_write_after_n_frames_once_and_the_writer_replays() {
        let inner = MemoryFrameSink::default();
        let events = inner.events.clone();
        let sink = ResetFrameSink::new(inner, 2);
        let (tx, rx) = mpsc::unbounded_channel();
        for f in [b"a", b"b", b"c", b"d"] {
            tx.send((f.to_vec(), None)).unwrap();
        }
        drop(tx);
        let in_flight = vec![b"a".to_vec(), b"b".to_vec()];
        run_ordered_writer(sink, rx, [0; 32], "t".into(), move || in_flight.clone()).await;
        let f = |b: &[u8]| Frame(b.to_vec());
        // "c" hit the reset: reopen, replay a and b, retry c, then d; no Closed.
        assert_eq!(
            *events.lock(),
            vec![Preamble, f(b"a"), f(b"b"), Preamble, f(b"a"), f(b"b"), f(b"c"), f(b"d")]
        );
    }
}
