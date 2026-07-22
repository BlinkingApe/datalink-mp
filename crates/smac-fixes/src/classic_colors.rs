//! Classic faction colors in multiplayer.
//!
//! **On by default.** Disable with `SMAC_NO_CLASSIC_COLORS=1`.
//!
//! ## The bug
//!
//! SMAC's correct colors follow faction identity (Spartans black, Hive blue,
//! ...). Single player shows them because faction *i* always sits in seat
//! *i*. In multiplayer the game seats factions by lobby position and random
//! fill, and the seat-indexed renderers (unit flags, base labels, minimap)
//! then paint each faction with the classic colors of whichever faction
//! canonically owns its seat NUMBER — while territory borders, which index
//! by canonical faction number (`tile[7]`), stay correct. Result: wrong unit
//! colors that also disagree with the borders.
//!
//! ## The fix (render-only; no game state touched)
//!
//! Physically re-seating factions would require permuting every seat-keyed
//! structure (rosters, DPID routing, sync state) — high corruption risk.
//! Instead, make the seat-indexed renderers produce faction colors:
//!
//! 1. Hook the moment seating is finalized: the sole call to
//!    `AssignPlayerFactions` (0x491630) at 0x491583 inside the game-start
//!    flow (runs for new AND loaded games, SP and MP).
//! 2. After it returns, rewrite the four 8-entry palette tables
//!    (0x6aeeb4/0x6aeed4/0x6aeef4/0x6aef14, sampled from PALETTE.PCX in
//!    canonical faction order) so that `table[seat] =
//!    original[faction_of(seat)]`, using the game's own seating table
//!    (0x917893, stride 0x17c). Every live reader — base labels, minimap,
//!    overlays — immediately shows each seat's true faction color.
//! 3. Re-run the game's unit-flag sprite stamper (0x45f3a0), which bakes
//!    the (now rewritten) table colors into the per-seat sprite copies.
//! 4. Install the border faction->seat translation (`border_fix`), so the
//!    border renderer indexes the rewritten tables by seat as well and
//!    borders keep their correct classic colors.
//!
//! In single player seating is identity, so the rewrite is a no-op. All
//! machines run the same DLL, so every machine renders identically. The
//! original table values are captured once (first game start in the
//! process, before any rewrite) and every rewrite derives from them, so
//! repeated games in one process stay correct.

#[cfg(target_os = "windows")]
use windows::Win32::System::Memory::{
    VirtualAlloc, VirtualProtect, MEM_COMMIT, MEM_RESERVE, PAGE_EXECUTE_READWRITE,
    PAGE_PROTECTION_FLAGS,
};

/// The four palette tables (primary, dark, text, shadow), 8 u32 entries each.
const TABLES: [usize; 4] = [0x6aeeb4, 0x6aeed4, 0x6aeef4, 0x6aef14];

/// Per-seat assigned-faction byte: 0x917893 + seat * 0x17c.
const SEATING_TABLE: usize = 0x917893;
const SEATING_STRIDE: usize = 0x17c;

/// The sole call site of AssignPlayerFactions, and its bytes.
const CALL_SITE: usize = 0x491583;
const CALL_ORIG: [u8; 5] = [0xe8, 0xa8, 0x00, 0x00, 0x00]; // call 0x491630

/// The functions we call from the stub / hook.
const ASSIGN_PLAYER_FACTIONS: usize = 0x491630;
const SPRITE_STAMPER: usize = 0x45f3a0;

/// Compute the rewritten tables: `out[t][seat] = orig[t][faction_of(seat) + 1]`.
///
/// The seating bytes (0x917893) are 0-BASED faction numbers (0=Gaians ..
/// 6=Peacekeepers; the RNG fill in AssignPlayerFactions draws from [0,7)),
/// while the palette tables put faction f at entry f+1 (entry 0 = natives).
/// Out-of-range bytes (empty seats, 0xff) fall back to the seat's own entry.
/// Seat 0 (natives) always keeps its original entry.
fn compute_tables(orig: &[[u32; 8]; 4], seating: &[u8; 8]) -> [[u32; 8]; 4] {
    let mut out = *orig;
    for t in 0..4 {
        for seat in 1..8 {
            let f = seating[seat] as usize;
            let src = if f <= 6 { f + 1 } else { seat };
            out[t][seat] = orig[t][src];
        }
    }
    out
}

/// Build the 15-byte detour stub for a stub placed at `stub_va`:
/// call AssignPlayerFactions (ecx/this is already set at the site), then the
/// Rust hook with all registers and flags preserved, then return to the site.
fn build_stub(stub_va: usize, hook: usize) -> [u8; 15] {
    let mut s = [0u8; 15];
    s[0] = 0xe8; // call ASSIGN_PLAYER_FACTIONS
    let rel = (ASSIGN_PLAYER_FACTIONS as i64 - (stub_va as i64 + 5)) as i32;
    s[1..5].copy_from_slice(&rel.to_le_bytes());
    s[5] = 0x9c; // pushfd
    s[6] = 0x60; // pushad
    s[7] = 0xe8; // call hook
    let rel = (hook as i64 - (stub_va as i64 + 12)) as i32;
    s[8..12].copy_from_slice(&rel.to_le_bytes());
    s[12] = 0x61; // popad
    s[13] = 0x9d; // popfd
    s[14] = 0xc3; // ret
    s
}

#[cfg(target_os = "windows")]
mod windows_impl {
    use super::*;
    use std::sync::OnceLock;
    use tracing::info;

    /// Original palette tables, captured on the first game start (pristine:
    /// the game samples them from PALETTE.PCX at init, before any rewrite).
    static ORIG: OnceLock<[[u32; 8]; 4]> = OnceLock::new();

    unsafe fn read_tables() -> [[u32; 8]; 4] {
        let mut out = [[0u32; 8]; 4];
        for (t, &addr) in TABLES.iter().enumerate() {
            std::ptr::copy_nonoverlapping(addr as *const u32, out[t].as_mut_ptr(), 8);
        }
        out
    }

    unsafe fn read_seating() -> [u8; 8] {
        let mut s = [0u8; 8];
        for (seat, v) in s.iter_mut().enumerate() {
            *v = *((SEATING_TABLE + seat * SEATING_STRIDE) as *const u8);
        }
        s
    }

    /// Called from the stub right after AssignPlayerFactions returns.
    pub unsafe extern "C" fn classic_colors_hook() {
        let orig = ORIG.get_or_init(|| read_tables());
        let seating = read_seating();
        let new = compute_tables(orig, &seating);
        for (t, &addr) in TABLES.iter().enumerate() {
            std::ptr::copy_nonoverlapping(new[t].as_ptr(), addr as *mut u32, 8);
        }
        // Rebake the per-seat unit-flag sprites from the rewritten tables.
        let restamp: extern "C" fn() = std::mem::transmute(SPRITE_STAMPER);
        restamp();
        info!(
            "classic colors: tables rewritten for seating {:?}, sprites restamped",
            &seating[1..]
        );
    }
}

/// Apply the classic-colors fix. On by default; `SMAC_NO_CLASSIC_COLORS=1`
/// disables it (and leaves the vanilla color behavior).
///
/// # Safety
///
/// Modifies executable code in the process. Call exactly once, early during
/// process init.
#[cfg(target_os = "windows")]
pub unsafe fn apply() -> Result<(), &'static str> {
    use tracing::info;

    if std::env::var("SMAC_NO_CLASSIC_COLORS").is_ok() {
        info!("classic colors: disabled via SMAC_NO_CLASSIC_COLORS");
        return Ok(());
    }

    // The border renderer must index by seat once the tables are rewritten.
    crate::border_fix::install_detour()?;

    let site = CALL_SITE as *mut u8;
    let actual = std::slice::from_raw_parts(site, CALL_ORIG.len());
    if actual != CALL_ORIG {
        if actual[0] == 0xe8 {
            // Some call is present but not the expected displacement.
            return Err("classic colors: unexpected call displacement - unknown binary version");
        }
        return Err("classic colors: original bytes don't match - unknown binary version");
    }

    let stub = VirtualAlloc(None, 15, MEM_COMMIT | MEM_RESERVE, PAGE_EXECUTE_READWRITE);
    if stub.is_null() {
        return Err("classic colors: failed to allocate stub");
    }
    let stub_va = stub as usize;
    let hook: unsafe extern "C" fn() = windows_impl::classic_colors_hook;
    let code = build_stub(stub_va, hook as usize);
    std::ptr::copy_nonoverlapping(code.as_ptr(), stub as *mut u8, code.len());

    let mut old_protect = PAGE_PROTECTION_FLAGS(0);
    if VirtualProtect(
        site as *const core::ffi::c_void,
        CALL_ORIG.len(),
        PAGE_EXECUTE_READWRITE,
        &mut old_protect,
    )
    .is_err()
    {
        return Err("classic colors: VirtualProtect failed");
    }
    let rel = (stub_va as i64 - (CALL_SITE as i64 + 5)) as i32;
    std::ptr::copy_nonoverlapping(rel.to_le_bytes().as_ptr(), site.add(1), 4);
    let mut dummy = PAGE_PROTECTION_FLAGS(0);
    let _ = VirtualProtect(site as *const core::ffi::c_void, CALL_ORIG.len(), old_protect, &mut dummy);

    info!(
        "classic colors: hooked AssignPlayerFactions call at 0x{:08X}, stub at 0x{:08X}",
        CALL_SITE, stub_va
    );
    Ok(())
}

/// Stub for non-Windows builds (testing).
#[cfg(not(target_os = "windows"))]
pub unsafe fn apply() -> Result<(), &'static str> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn orig() -> [[u32; 8]; 4] {
        // Distinct sentinel per (table, entry)
        let mut o = [[0u32; 8]; 4];
        for t in 0..4 {
            for i in 0..8 {
                o[t][i] = (t as u32) << 8 | i as u32;
            }
        }
        o
    }

    #[test]
    fn identity_seating_is_noop() {
        let o = orig();
        // SP identity: seat i holds 0-based faction i-1 -> palette entry i
        let seating = [0, 0, 1, 2, 3, 4, 5, 6];
        assert_eq!(compute_tables(&o, &seating), o);
    }

    #[test]
    fn permuted_seating_maps_to_faction_colors() {
        let o = orig();
        // test0036: Spartans (0-based faction 4) in seat 2, Gaians (0) in seat 1
        let mut seating = [0, 0, 1, 2, 3, 4, 5, 6];
        seating[2] = 4; // Spartans
        seating[5] = 1; // Hive
        let n = compute_tables(&o, &seating);
        for t in 0..4 {
            assert_eq!(n[t][2], o[t][5], "seat 2 wears Spartan (palette 5) colors");
            assert_eq!(n[t][5], o[t][2], "seat 5 wears Hive (palette 2) colors");
            assert_eq!(n[t][1], o[t][1], "seat 1 unchanged (Gaians in seat 1)");
            assert_eq!(n[t][0], o[t][0], "seat 0 (natives) untouched");
        }
    }

    #[test]
    fn live_seating_2026_07_21() {
        // Observed live: seating [4,1,0,2,6,3,5] for seats 1..7 (0-based factions)
        let o = orig();
        let seating = [0, 4, 1, 0, 2, 6, 3, 5];
        let n = compute_tables(&o, &seating);
        for t in 0..4 {
            assert_eq!(n[t][1], o[t][5], "seat 1 = Spartans -> black (palette 5)");
            assert_eq!(n[t][2], o[t][2], "seat 2 = Hive -> blue (palette 2)");
            assert_eq!(n[t][3], o[t][1], "seat 3 = Gaians -> green (palette 1)");
        }
    }

    #[test]
    fn out_of_range_faction_falls_back_to_seat() {
        let o = orig();
        // 0xff = unset; byte 7+ out of range for 7 factions
        let seating = [0, 0xff, 0xff, 7, 0xff, 0xff, 0xff, 0xff];
        let n = compute_tables(&o, &seating);
        assert_eq!(n, o, "unset seats keep their own entries");
    }

    #[test]
    fn stub_calls_land_on_targets() {
        let stub_va = 0x30000000usize;
        let hook = 0x10001234usize;
        let s = build_stub(stub_va, hook);
        assert_eq!(s.len(), 15);
        assert_eq!(s[0], 0xe8);
        let rel = i32::from_le_bytes(s[1..5].try_into().unwrap());
        assert_eq!((stub_va + 5) as i64 + rel as i64, ASSIGN_PLAYER_FACTIONS as i64);
        assert_eq!(s[7], 0xe8);
        let rel = i32::from_le_bytes(s[8..12].try_into().unwrap());
        assert_eq!((stub_va + 12) as i64 + rel as i64, hook as i64);
        assert_eq!(&s[5..7], &[0x9c, 0x60], "pushfd/pushad before hook");
        assert_eq!(&s[12..15], &[0x61, 0x9d, 0xc3], "popad/popfd/ret");
    }
}
