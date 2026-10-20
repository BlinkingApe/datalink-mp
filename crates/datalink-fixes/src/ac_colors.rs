//! Classic faction colors for Alien Crossfire (`terranx.exe`).
//!
//! Unlike SMAC, Alien Crossfire reads each seat's colors from that seat's own
//! faction art file (`PIRATES.pcx`, ...) through a function the game already
//! has. So the fix does not rewrite palette tables. After the game finalizes
//! seating, it makes the game redo its own per-seat load for every seated
//! faction and then re-stamp the flag sprites. Borders index the same slot as
//! the flags, so no border detour is needed. See
//! `docs/contributors/terranx-faction-colors.md`.
//!
//! This file holds the pure decisions (which seats to reload, how to read a
//! faction file name, what to log); `classic_colors.rs` installs the hook.

/// Longest faction file name stem the game stores (`MFaction.filename`).
pub(crate) const FILENAME_LEN: usize = 24;

/// Read a faction file name from the raw `MFaction.filename` bytes.
///
/// The name ends at the first NUL. An empty or unprintable name means the
/// seat holds no faction.
pub(crate) fn parse_filename(raw: &[u8]) -> Option<String> {
    let end = raw.iter().position(|&b| b == 0).unwrap_or(raw.len());
    let name = &raw[..end];
    if name.is_empty() || !name.iter().all(|b| b.is_ascii_graphic()) {
        return None;
    }
    Some(String::from_utf8_lossy(name).into_owned())
}

/// True when every seat 1 to 7 holds the faction of its own number, which is
/// how single player seats (and the startup default order).
pub(crate) fn is_identity_seating(seating: &[u8; 8]) -> bool {
    (1..8).all(|seat| seating[seat] as usize == seat - 1)
}

/// The seats (1 to 7) whose faction art the game should load again.
///
/// Single player (identity seating) is a no-op: the startup load already left
/// the right colors. Otherwise every seat that names a faction file is
/// reloaded, so its color table entries match the file it actually holds.
pub(crate) fn plan_reloads(seating: &[u8; 8], names: &[Option<String>; 8]) -> Vec<usize> {
    if is_identity_seating(seating) {
        return Vec::new();
    }
    (1..8).filter(|&seat| names[seat].is_some()).collect()
}

/// One log line for a seat: its seating byte and faction file, and whether
/// the fix reloads it.
pub(crate) fn seat_log_line(
    seat: usize,
    seating_byte: u8,
    name: &Option<String>,
    reload: bool,
) -> String {
    format!(
        "seat {seat}: seating byte {seating_byte}, faction file {}, {}",
        name.as_deref().unwrap_or("(none)"),
        if reload { "colors reloaded" } else { "left alone" }
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const IDENTITY: [u8; 8] = [0, 0, 1, 2, 3, 4, 5, 6];

    fn names(list: &[&str]) -> [Option<String>; 8] {
        let mut n: [Option<String>; 8] = Default::default();
        for (i, s) in list.iter().enumerate() {
            if !s.is_empty() {
                n[i + 1] = Some(s.to_string());
            }
        }
        n
    }

    #[test]
    fn single_player_is_a_noop() {
        let n = names(&["GAIANS", "HIVE", "UNIV", "MORGAN", "SPARTANS", "BELIEVERS", "PEACE"]);
        assert!(plan_reloads(&IDENTITY, &n).is_empty());
    }

    #[test]
    fn crossfire_pirates_in_sixth_seat_is_reloaded() {
        // Pirates seated 6th, as in the 2026-10-20 game.
        let mut seating = IDENTITY;
        seating[6] = 9;
        let n = names(&["GAIANS", "HIVE", "UNIV", "MORGAN", "SPARTANS", "PIRATES", "PEACE"]);
        assert_eq!(plan_reloads(&seating, &n), vec![1, 2, 3, 4, 5, 6, 7]);
    }

    #[test]
    fn two_player_crossfire_game_reloads_only_named_seats() {
        let seating = [0, 4, 8, 0xff, 0xff, 0xff, 0xff, 0xff];
        let n = names(&["SPARTANS", "CARETAKE"]);
        assert_eq!(plan_reloads(&seating, &n), vec![1, 2]);
    }

    #[test]
    fn permuted_smac_factions_are_reloaded_too() {
        let seating = [0, 4, 1, 0, 2, 6, 3, 5];
        let n = names(&["SPARTANS", "HIVE", "GAIANS", "UNIV", "PEACE", "MORGAN", "BELIEVER"]);
        assert_eq!(plan_reloads(&seating, &n).len(), 7);
    }

    #[test]
    fn identity_detection() {
        assert!(is_identity_seating(&IDENTITY));
        let mut s = IDENTITY;
        s[3] = 5;
        assert!(!is_identity_seating(&s));
    }

    #[test]
    fn filename_parsing() {
        assert_eq!(parse_filename(b"PIRATES\0junk"), Some("PIRATES".into()));
        assert_eq!(parse_filename(b"\0PIRATES"), None);
        assert_eq!(parse_filename(b"BAD\x01NAME\0"), None);
        assert_eq!(
            parse_filename(b"EXACT24CHARACTERSLONG__"),
            Some("EXACT24CHARACTERSLONG__".into())
        );
    }

    #[test]
    fn log_line_names_byte_file_and_action() {
        let l = seat_log_line(6, 9, &Some("PIRATES".into()), true);
        assert_eq!(l, "seat 6: seating byte 9, faction file PIRATES, colors reloaded");
        assert!(seat_log_line(3, 0xff, &None, false).contains("(none), left alone"));
    }
}
