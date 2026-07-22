//! Optional recolor of territory borders to match unit colors in multiplayer.
//!
//! **Off by default.** Enable with `SMAC_BORDER_MATCH_UNITS=1`.
//!
//! ## Background
//!
//! SMAC's design-correct colors follow FACTION IDENTITY (Spartans black,
//! Hive blue, ...), and single player always shows them because faction *i*
//! sits in seat *i*. In multiplayer the game re-seats factions, and the
//! renderers split: unit flags, base labels, and minimap dots index the
//! palette by SEAT (so Spartans seated 2nd wear Hive's blue — wrong), while
//! territory borders index by the tile owner byte (`tile[7]`), which holds
//! the canonical faction number — so borders keep the CORRECT classic
//! colors. The real bug is the seat-indexed unit coloring; the planned fix
//! is identity seating (faction i -> seat i), after which every renderer
//! agrees and this module is a no-op.
//!
//! Until then, this optional patch makes the borders match the (wrong)
//! unit colors, trading correctness for on-screen consistency. Some players
//! may prefer that while playing; most should leave it off.
//!
//! ## Mechanism
//!
//! Detours the border-color path in the map renderer at 0x470d2a:
//!
//! ```text
//! 0x470d2a  mov eax,[ebp-0x44]   ; owner = faction number   <- detour here
//! 0x470d2d  test eax,eax
//! 0x470d2f  jle  0x470d3e        ; unowned -> other coloring
//! 0x470d31  mov eax,[eax*4+0x6aeeb4]  ; palette[owner]         (kept)
//! ```
//!
//! The stub reloads the owner and, when it is a faction number in 1..=7,
//! scans the per-seat assigned-faction table (0x917893, stride 0x17c) for
//! the seat holding that faction, substituting the seat index before the
//! untouched palette load. Unmatched values (neutral/native) pass through.
//! The second entry into the palette load (via 0x470d12) belongs to a
//! different, non-border coloring path and is left alone. No synced game
//! state is touched.

#[cfg(target_os = "windows")]
use windows::Win32::System::Memory::{
    VirtualAlloc, VirtualProtect, MEM_COMMIT, MEM_RESERVE, PAGE_EXECUTE_READWRITE,
    PAGE_PROTECTION_FLAGS,
};

/// Detour site: `mov eax,[ebp-0x44]; test eax,eax; jle +0x0d` (7 bytes).
const SITE_ADDR: usize = 0x470d2a;
const SITE_ORIG: [u8; 7] = [0x8b, 0x45, 0xbc, 0x85, 0xc0, 0x7e, 0x0d];

/// Where the stub jumps back to: the original palette load.
const CONTINUE_ADDR: usize = 0x470d31;
/// Where the original `jle` went: the not-a-border coloring path.
const SKIP_ADDR: usize = 0x470d3e;

/// Per-slot assigned-faction byte: 0x917893 + slot * 0x17c.
const SEATING_TABLE: u32 = 0x917893;
const SEATING_STRIDE: u32 = 0x17c;

/// Offsets of the two rel32 operands in the stub (see `build_stub`).
const JLE_OPERAND_OFF: usize = 52;
const JMP_OPERAND_OFF: usize = 57;
const STUB_LEN: usize = 61;

/// Assemble the detour stub for a stub placed at `stub_va`.
///
/// The incoming owner (`tile[7]`) is a 1-BASED faction number (1..=7), while
/// the seating-table bytes are 0-BASED faction numbers — hence the `dec eax`
/// before the scan and the `inc eax` restore on the not-found path.
fn build_stub(stub_va: usize) -> Vec<u8> {
    let mut s = Vec::with_capacity(STUB_LEN);
    s.extend_from_slice(&[0x8b, 0x45, 0xbc]); //  0: mov eax,[ebp-0x44]
    s.push(0x51); //                              3: push ecx
    s.push(0x52); //                              4: push edx
    s.extend_from_slice(&[0x83, 0xf8, 0x01]); //  5: cmp eax,1
    s.extend_from_slice(&[0x7c, 0x24]); //        8: jl  .skip (46)
    s.extend_from_slice(&[0x83, 0xf8, 0x07]); // 10: cmp eax,7
    s.extend_from_slice(&[0x7f, 0x1f]); //       13: jg  .skip (46)
    s.push(0x48); //                             15: dec eax (1-based -> 0-based)
    s.extend_from_slice(&[0xb9, 0x07, 0x00, 0x00, 0x00]); // 16: mov ecx,7
    // .scan (21):
    s.extend_from_slice(&[0x69, 0xd1]); //       21: imul edx,ecx,SEATING_STRIDE
    s.extend_from_slice(&SEATING_STRIDE.to_le_bytes());
    s.extend_from_slice(&[0x0f, 0xb6, 0x92]); // 27: movzx edx, byte [edx+SEATING_TABLE]
    s.extend_from_slice(&SEATING_TABLE.to_le_bytes());
    s.extend_from_slice(&[0x39, 0xc2]); //       34: cmp edx,eax
    s.extend_from_slice(&[0x74, 0x06]); //       36: je  .found (44)
    s.push(0x49); //                             38: dec ecx
    s.extend_from_slice(&[0x75, 0xec]); //       39: jnz .scan (21)
    s.push(0x40); //                             41: inc eax (restore 1-based)
    s.extend_from_slice(&[0xeb, 0x02]); //       42: jmp .skip (46)
    s.extend_from_slice(&[0x89, 0xc8]); //       44: .found: mov eax,ecx
    s.push(0x5a); //                             46: .skip:  pop edx
    s.push(0x59); //                             47: pop ecx
    s.extend_from_slice(&[0x85, 0xc0]); //       48: test eax,eax
    s.extend_from_slice(&[0x0f, 0x8e]); //       50: jle SKIP_ADDR
    let jle_rel = (SKIP_ADDR as i64 - (stub_va as i64 + JLE_OPERAND_OFF as i64 + 4)) as i32;
    s.extend_from_slice(&jle_rel.to_le_bytes());
    s.push(0xe9); //                             56: jmp CONTINUE_ADDR
    let jmp_rel = (CONTINUE_ADDR as i64 - (stub_va as i64 + JMP_OPERAND_OFF as i64 + 4)) as i32;
    s.extend_from_slice(&jmp_rel.to_le_bytes());
    debug_assert_eq!(s.len(), STUB_LEN);
    s
}

/// Apply the standalone border recolor option (OFF unless
/// `SMAC_BORDER_MATCH_UNITS=1`). The classic-colors fix installs the detour
/// directly via [`install_detour`] regardless of this option.
///
/// # Safety
///
/// Modifies executable code in the process. Call exactly once, early during
/// process init, before the map renderer runs.
#[cfg(target_os = "windows")]
pub unsafe fn apply() -> Result<(), &'static str> {
    use tracing::info;

    if std::env::var("SMAC_BORDER_MATCH_UNITS").map(|v| v != "0").unwrap_or(false) == false {
        info!("border recolor: not requested standalone (classic-colors fix may still install it)");
        return Ok(());
    }
    install_detour()
}

/// Install the faction->seat border detour (idempotent).
///
/// # Safety
///
/// Modifies executable code in the process.
#[cfg(target_os = "windows")]
pub(crate) unsafe fn install_detour() -> Result<(), &'static str> {
    use tracing::info;

    let site = SITE_ADDR as *mut u8;
    let actual = std::slice::from_raw_parts(site, SITE_ORIG.len());
    if actual[0] == 0xe9 {
        info!("border fix: already patched at 0x{:08X}", SITE_ADDR);
        return Ok(());
    }
    if actual != SITE_ORIG {
        return Err("border fix: original bytes don't match - unknown binary version");
    }

    // Allocate and fill the stub.
    let stub = VirtualAlloc(None, STUB_LEN, MEM_COMMIT | MEM_RESERVE, PAGE_EXECUTE_READWRITE);
    if stub.is_null() {
        return Err("border fix: failed to allocate stub");
    }
    let stub_va = stub as usize;
    let code = build_stub(stub_va);
    std::ptr::copy_nonoverlapping(code.as_ptr(), stub as *mut u8, code.len());

    // Patch the site: jmp stub + 2 NOPs.
    let mut old_protect = PAGE_PROTECTION_FLAGS(0);
    if VirtualProtect(
        site as *const core::ffi::c_void,
        SITE_ORIG.len(),
        PAGE_EXECUTE_READWRITE,
        &mut old_protect,
    )
    .is_err()
    {
        return Err("border fix: VirtualProtect failed");
    }
    let rel = (stub_va as i64 - (SITE_ADDR as i64 + 5)) as i32;
    *site = 0xe9;
    std::ptr::copy_nonoverlapping(rel.to_le_bytes().as_ptr(), site.add(1), 4);
    *site.add(5) = 0x90;
    *site.add(6) = 0x90;
    let mut dummy = PAGE_PROTECTION_FLAGS(0);
    let _ = VirtualProtect(site as *const core::ffi::c_void, SITE_ORIG.len(), old_protect, &mut dummy);

    info!(
        "Patched territory-border color (faction->slot translation) at 0x{:08X}, stub at 0x{:08X}",
        SITE_ADDR, stub_va
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

    #[test]
    fn stub_layout_and_length() {
        let s = build_stub(0x10000000);
        assert_eq!(s.len(), STUB_LEN);
        // Prefix must replicate the original load so the game sees the same value.
        assert_eq!(&s[0..3], &SITE_ORIG[0..3]);
    }

    #[test]
    fn short_jumps_land_on_labels() {
        let s = build_stub(0x10000000);
        // jl at 8 (operand 9, next ip 10) -> .skip at 46
        assert_eq!(s[8], 0x7c);
        assert_eq!(10 + s[9] as i8 as i32, 46);
        // jg at 13 (next ip 15) -> .skip at 46
        assert_eq!(s[13], 0x7f);
        assert_eq!(15 + s[14] as i8 as i32, 46);
        // dec eax (1-based owner -> 0-based seating byte) then scan
        assert_eq!(s[15], 0x48);
        // je at 36 (next ip 38) -> .found at 44
        assert_eq!(s[36], 0x74);
        assert_eq!(38 + s[37] as i8 as i32, 44);
        // jnz at 39 (next ip 41) -> .scan at 21
        assert_eq!(s[39], 0x75);
        assert_eq!(41 + s[40] as i8 as i32, 21);
        // inc eax restore, then jmp at 42 (next ip 44) -> .skip at 46
        assert_eq!(s[41], 0x40);
        assert_eq!(s[42], 0xeb);
        assert_eq!(44 + s[43] as i8 as i32, 46);
    }

    #[test]
    fn tail_jumps_target_game_addresses() {
        let stub_va = 0x20000000usize;
        let s = build_stub(stub_va);
        let jle_rel = i32::from_le_bytes(s[JLE_OPERAND_OFF..JLE_OPERAND_OFF + 4].try_into().unwrap());
        assert_eq!(
            (stub_va + JLE_OPERAND_OFF + 4) as i64 + jle_rel as i64,
            SKIP_ADDR as i64
        );
        let jmp_rel = i32::from_le_bytes(s[JMP_OPERAND_OFF..JMP_OPERAND_OFF + 4].try_into().unwrap());
        assert_eq!(
            (stub_va + JMP_OPERAND_OFF + 4) as i64 + jmp_rel as i64,
            CONTINUE_ADDR as i64
        );
    }

    /// Emulate the stub's translation logic against a model seating table.
    #[test]
    fn translation_semantics() {
        // Model of the stub: owner is a 1-BASED faction (tile[7]); seating
        // bytes are 0-BASED factions; scan seats 7..=1, first match wins.
        fn translate(owner: i32, seating: &[u8; 8]) -> i32 {
            if !(1..=7).contains(&owner) {
                return owner;
            }
            let want = (owner - 1) as u8; // 0-based faction byte
            for seat in (1..=7).rev() {
                if seating[seat] == want {
                    return seat as i32;
                }
            }
            owner
        }
        // test0036 seating: Gaians (byte 0) in seat 1, Spartans (byte 4) in seat 2
        let mut seating = [0xffu8; 8];
        seating[1] = 0;
        seating[2] = 4;
        assert_eq!(translate(5, &seating), 2, "Spartan border (tile 5) -> seat 2");
        assert_eq!(translate(1, &seating), 1, "Gaian border (tile 1) -> seat 1");
        assert_eq!(translate(6, &seating), 6, "unseated faction passes through");
        assert_eq!(translate(0, &seating), 0, "neutral passes through");
        assert_eq!(translate(-1, &seating), -1, "unowned passes through");
    }
}
