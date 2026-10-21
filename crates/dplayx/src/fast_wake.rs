//! `DATALINK_FAST_WAKE` switch: timing only, no IPC change.
//!
//! With the switch on, the DLL signals the game's event right after each
//! `Send` returns and the poll thread sleeps 1 ms instead of 10 between
//! inbound-message checks. See ADR-0005 section 3.

use std::sync::OnceLock;
use std::time::Duration;

/// Poll interval with the switch off (today's behaviour).
pub const SLOW_POLL: Duration = Duration::from_millis(10);
/// Poll interval with the switch on.
pub const FAST_POLL: Duration = Duration::from_millis(1);

/// Parse the raw env value: only `1` turns the switch on.
pub fn parse(value: Option<&str>) -> bool {
    value == Some("1")
}

/// Poll sleep for the given switch state.
pub fn poll_interval(enabled: bool) -> Duration {
    if enabled {
        FAST_POLL
    } else {
        SLOW_POLL
    }
}

static ENABLED: OnceLock<bool> = OnceLock::new();

/// Switch state, read from the environment once and cached.
pub fn enabled() -> bool {
    *ENABLED.get_or_init(|| parse(std::env::var("DATALINK_FAST_WAKE").ok().as_deref()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn off_by_default_and_for_other_values() {
        assert!(!parse(None));
        assert!(!parse(Some("")));
        assert!(!parse(Some("0")));
        assert!(!parse(Some("true")));
    }

    #[test]
    fn on_only_for_one() {
        assert!(parse(Some("1")));
    }

    #[test]
    fn poll_is_ten_ms_off_and_one_ms_on() {
        assert_eq!(poll_interval(false), Duration::from_millis(10));
        assert_eq!(poll_interval(true), Duration::from_millis(1));
    }
}
