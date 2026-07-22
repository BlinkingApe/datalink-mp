//! Fix for the backwards (STD) blit remainder bug in terran_PRACX.exe.
//!
//! ## Where
//!
//! One sprite blit routine (0x5f9040..0x5f959b) contains FIVE copy loops: three
//! forward (`CLD`) variants and two backward (`STD`) variants, selected by the
//! horizontal/vertical flip and clip situation. Each loop copies a row as
//! `q = width>>2` dwords (`REP MOVSD`) plus `r = width&3` trailing bytes
//! (`REP MOVSB`), advancing by the row pitch between rows.
//!
//! The two BACKWARD loops are buggy; the three forward loops are correct.
//!
//! | loop | addr    | row adjust        | pitch term |
//! |------|---------|-------------------|------------|
//! | A    | 0x5f9333| `add esi,edx`     | pitch + W  |
//! | B    | 0x5f9559| `sub esi,edx`     | pitch - W  |
//!
//! (An earlier version of this fix patched only loop B, leaving loop A — one of
//! the two flip directions — still corrupting sprites.)
//!
//! ## The bug
//!
//! For a backward copy the source pointer is set up pointing at the LOW byte of
//! the row's TOP dword (`rowlow + W - 4`); `REP MOVSD` with `STD` then walks the
//! dwords downward. The remaining `r` low bytes must be copied afterwards, and
//! because `MOVSD` left `esi` at `rowlow + r - 4` while `MOVSB` needs to start at
//! `rowlow + r - 1`, the pointer must be nudged `+3` between the two `REP`s.
//!
//! Instead the original code does `add esi, ecx` (i.e. `+r`) BEFORE the loop.
//! That makes `MOVSD` read starting at `rowlow + W - 4 + r` — reading up to
//! `r` bytes past the row end — and leaves the `r` remainder bytes copied from
//! 3 bytes below where they belong, with a 3-byte hole in the middle. The result
//! is only correct when `r == 0` (width a multiple of 4), which is why the base
//! game looks fine for most sprites and only glitches on odd-width ones.
//!
//! ## The previous fix and why it was still wrong
//!
//! The prior patch used the correct `+3` idea but turned the per-row
//! `mov ecx, ebp` (reload dword count) into a one-time `xchg ecx, ebp` and made
//! the loop re-enter at `rep movsd`. Because `rep movsb` leaves `ecx == 0`, every
//! row AFTER the first copied ZERO dwords — so it fixed single-row blits but
//! corrupted every taller sprite (including the multiple-of-4 widths the original
//! handled fine). That is the "visual issues" regression.
//!
//! ## This fix
//!
//! For each backward loop we rewrite the loop and a little of its setup in place
//! (there is room up to the shared `pop ebp` epilogue) so that:
//!   * `ebp` holds the full width `W` for the whole loop (persistent);
//!   * each iteration recomputes `q = ebp>>2` and `r = ebp&3` into `ecx`
//!     (so both counts are always fresh — no stale-`ecx` bug);
//!   * `esi/edi` start at the top dword (no bogus pre-add), get `+3` between the
//!     `MOVSD` and `MOVSB`, and the constant `3` is folded into the row-adjust
//!     term (`±3` baked into `edx`/`ebx` once) so no extra per-row instructions
//!     are needed.
//!
//! Verified: an instruction-level emulation of both rewritten loops reproduces a
//! byte-exact row copy for every width 1..=19 and heights 1/2/5, with no
//! out-of-bounds access. The forward loops are left untouched.

#[cfg(target_os = "windows")]
use windows::Win32::System::Memory::{VirtualProtect, PAGE_EXECUTE_READWRITE, PAGE_PROTECTION_FLAGS};

/// A single in-place patch: expected original bytes and the replacement.
struct Patch {
    name: &'static str,
    addr: usize,
    original: &'static [u8],
    fixed: &'static [u8],
}

// ---- Loop B @ 0x5f9544 (setup) .. 0x5f9576 (pop ebp), 50 bytes ----
// Original: sub ebx,[ebp+1c]; mov edx,[ebp+c]; sub edx,[ebp+1c]; mov ecx,[ebp+1c];
//   push ebp; mov ebp,ecx; shr ebp,2; and ecx,3; mov [9bbecc],ecx;
//   add esi,ecx; add edi,ecx; L: mov ecx,ebp; rep movsd; mov ecx,[9bbecc];
//   rep movsb; sub esi,edx; sub edi,ebx; dec eax; jne L
const LOOP_B_ADDR: usize = 0x005F9544;
const LOOP_B_ORIG: [u8; 50] = [
    0x2b, 0x5d, 0x1c, 0x8b, 0x55, 0x0c, 0x2b, 0x55, 0x1c, 0x8b, 0x4d, 0x1c, 0x55, 0x8b, 0xe9, 0xc1,
    0xed, 0x02, 0x83, 0xe1, 0x03, 0x89, 0x0d, 0xcc, 0xbe, 0x9b, 0x00, 0x03, 0xf1, 0x03, 0xf9, 0x8b,
    0xcd, 0xf3, 0xa5, 0x8b, 0x0d, 0xcc, 0xbe, 0x9b, 0x00, 0xf3, 0xa4, 0x2b, 0xf2, 0x2b, 0xfb, 0x48,
    0x75, 0xed,
];
// Fixed: sub ebx,[ebp+1c]; add ebx,3; mov edx,[ebp+c]; sub edx,[ebp+1c]; add edx,3;
//   mov ecx,[ebp+1c]; push ebp; mov ebp,ecx;
//   L: mov ecx,ebp; shr ecx,2; rep movsd; add esi,3; add edi,3; mov ecx,ebp; and ecx,3;
//   rep movsb; sub esi,edx; sub edi,ebx; dec eax; jne L; nop; nop
const LOOP_B_FIXED: [u8; 50] = [
    0x2b, 0x5d, 0x1c, 0x83, 0xc3, 0x03, 0x8b, 0x55, 0x0c, 0x2b, 0x55, 0x1c, 0x83, 0xc2, 0x03, 0x8b,
    0x4d, 0x1c, 0x55, 0x8b, 0xe9, 0x8b, 0xcd, 0xc1, 0xe9, 0x02, 0xf3, 0xa5, 0x83, 0xc6, 0x03, 0x83,
    0xc7, 0x03, 0x8b, 0xcd, 0x83, 0xe1, 0x03, 0xf3, 0xa4, 0x2b, 0xf2, 0x2b, 0xfb, 0x48, 0x75, 0xe5,
    0x90, 0x90,
];

// ---- Loop A @ 0x5f931e (setup) .. 0x5f9350 (pop ebp), 50 bytes ----
// Same shape as B but row adjust is `add` and the pitch term is `+W`, so the
// folded constant is `-3` (sub ebx,3 / sub edx,3) instead of `+3`.
const LOOP_A_ADDR: usize = 0x005F931E;
const LOOP_A_ORIG: [u8; 50] = [
    0x03, 0x5d, 0x1c, 0x8b, 0x55, 0x0c, 0x03, 0x55, 0x1c, 0x8b, 0x4d, 0x1c, 0x55, 0x8b, 0xe9, 0xc1,
    0xed, 0x02, 0x83, 0xe1, 0x03, 0x89, 0x0d, 0xcc, 0xbe, 0x9b, 0x00, 0x03, 0xf1, 0x03, 0xf9, 0x8b,
    0xcd, 0xf3, 0xa5, 0x8b, 0x0d, 0xcc, 0xbe, 0x9b, 0x00, 0xf3, 0xa4, 0x03, 0xf2, 0x03, 0xfb, 0x48,
    0x75, 0xed,
];
const LOOP_A_FIXED: [u8; 50] = [
    0x03, 0x5d, 0x1c, 0x83, 0xeb, 0x03, 0x8b, 0x55, 0x0c, 0x03, 0x55, 0x1c, 0x83, 0xea, 0x03, 0x8b,
    0x4d, 0x1c, 0x55, 0x8b, 0xe9, 0x8b, 0xcd, 0xc1, 0xe9, 0x02, 0xf3, 0xa5, 0x83, 0xc6, 0x03, 0x83,
    0xc7, 0x03, 0x8b, 0xcd, 0x83, 0xe1, 0x03, 0xf3, 0xa4, 0x03, 0xf2, 0x03, 0xfb, 0x48, 0x75, 0xe5,
    0x90, 0x90,
];

const PATCHES: [Patch; 2] = [
    Patch { name: "backwards-blit loop A", addr: LOOP_A_ADDR, original: &LOOP_A_ORIG, fixed: &LOOP_A_FIXED },
    Patch { name: "backwards-blit loop B", addr: LOOP_B_ADDR, original: &LOOP_B_ORIG, fixed: &LOOP_B_FIXED },
];

/// Apply both backwards-blit fixes.
///
/// # Safety
///
/// Modifies executable code in the process. Call exactly once, early during
/// process init, before the sprite blitter runs.
#[cfg(target_os = "windows")]
pub unsafe fn apply() -> Result<(), &'static str> {
    for p in &PATCHES {
        apply_one(p)?;
    }
    Ok(())
}

#[cfg(target_os = "windows")]
unsafe fn apply_one(p: &Patch) -> Result<(), &'static str> {
    use tracing::{debug, info};

    let ptr = p.addr as *const u8;
    let len = p.fixed.len();
    debug_assert_eq!(p.original.len(), len);

    let actual = std::slice::from_raw_parts(ptr, len);
    if actual == p.fixed {
        info!("{} already patched at 0x{:08X}", p.name, p.addr);
        return Ok(());
    }
    if actual != p.original {
        debug!(
            "{}: original bytes don't match at 0x{:08X}. expected {:02x?}, got {:02x?}",
            p.name, p.addr, p.original, actual
        );
        return Err("blit fix: original bytes don't match - unknown binary version");
    }

    let mut old_protect = PAGE_PROTECTION_FLAGS(0);
    if VirtualProtect(
        ptr as *const core::ffi::c_void,
        len,
        PAGE_EXECUTE_READWRITE,
        &mut old_protect,
    )
    .is_err()
    {
        return Err("blit fix: VirtualProtect failed to make code writable");
    }

    std::ptr::copy_nonoverlapping(p.fixed.as_ptr(), p.addr as *mut u8, len);

    let mut dummy = PAGE_PROTECTION_FLAGS(0);
    let _ = VirtualProtect(ptr as *const core::ffi::c_void, len, old_protect, &mut dummy);

    info!("Patched {} ({} bytes) at 0x{:08X}", p.name, len, p.addr);
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
    fn windows_are_50_bytes_and_end_before_pop_ebp() {
        for p in &PATCHES {
            assert_eq!(p.original.len(), 50, "{}", p.name);
            assert_eq!(p.fixed.len(), 50, "{}", p.name);
        }
        // pop ebp (0x5d) lives immediately after each window.
        assert_eq!(LOOP_A_ADDR + 50, 0x005F9350);
        assert_eq!(LOOP_B_ADDR + 50, 0x005F9576);
    }

    #[test]
    fn jne_targets_the_loop_label() {
        // The loop label L is at window offset 21 (`mov ecx, ebp`), and the
        // `jne rel8` is at offset 46 with the operand at 47 (next ip = 48).
        for f in [&LOOP_A_FIXED, &LOOP_B_FIXED] {
            assert_eq!(f[46], 0x75, "jne opcode");
            let rel = f[47] as i8 as i32;
            let target = 48 + rel; // next_ip(offset 48) + rel
            assert_eq!(target, 21, "jne must land on the loop label");
        }
    }

    #[test]
    fn fixed_reloads_counts_each_iteration() {
        // Regression guard against the stale-ecx bug: the byte just before
        // `rep movsd` (f3 a5) must be part of a `shr ecx,2` (…c1 e9 02) so the
        // dword count is recomputed every iteration rather than clobbered.
        for f in [&LOOP_A_FIXED, &LOOP_B_FIXED] {
            let movsd = f.windows(2).position(|w| w == [0xf3, 0xa5]).unwrap();
            assert_eq!(&f[movsd - 3..movsd], &[0xc1, 0xe9, 0x02], "shr ecx,2 before rep movsd");
        }
    }
}
