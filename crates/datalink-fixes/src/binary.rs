//! Which game binary the DLL is running inside.
//!
//! Every patch address in this crate belongs to one build of the game, so
//! the DLL works out the binary first and applies only the patches that
//! exist for it. The binary is recognized from the PE header of the image at
//! the game's load address (`GAME_IMAGE_BASE`), which stays intact whatever
//! launcher started the game (`terranx.exe`, `terranx_PRACX.exe`, WTP) and
//! whatever it patches in memory afterwards.

use std::fmt;

/// Load address of the game image. All patch addresses assume it.
pub(crate) const GAME_IMAGE_BASE: usize = 0x0040_0000;

/// How many bytes from `GAME_IMAGE_BASE` `identify` needs.
pub(crate) const HEADER_LEN: usize = 0x200;

/// A game binary this crate knows addresses for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GameBinary {
    /// Alpha Centauri, `terran.exe` (and `terran_PRACX.exe`, same image).
    Smac,
    /// Alien Crossfire v2.0, `terranx.exe` (and `terranx_PRACX.exe`, same
    /// image; also the image WTP's `terranx_mod.exe` carries).
    AlienCrossfire,
}

impl fmt::Display for GameBinary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            GameBinary::Smac => "SMAC (terran.exe)",
            GameBinary::AlienCrossfire => "Alien Crossfire (terranx.exe)",
        })
    }
}

/// PE fields that tell the builds apart.
struct Signature {
    binary: GameBinary,
    time_date_stamp: u32,
    size_of_image: u32,
    entry_point_rva: u32,
}

const SIGNATURES: [Signature; 2] = [
    Signature {
        binary: GameBinary::Smac,
        time_date_stamp: 0x3d59_0cc5,
        size_of_image: 0x60f000,
        entry_point_rva: 0x2616bd,
    },
    Signature {
        binary: GameBinary::AlienCrossfire,
        time_date_stamp: 0x385e_91ff,
        size_of_image: 0x60c000,
        entry_point_rva: 0x246c9d,
    },
];

fn u32_at(bytes: &[u8], at: usize) -> Option<u32> {
    let end = at.checked_add(4)?;
    Some(u32::from_le_bytes(bytes.get(at..end)?.try_into().ok()?))
}

/// Recognize the game binary from the start of its image (the DOS and PE
/// headers). `None` means not a binary this crate knows.
pub(crate) fn identify(header: &[u8]) -> Option<GameBinary> {
    if header.get(..2)? != b"MZ" {
        return None;
    }
    let pe = u32_at(header, 0x3c)? as usize;
    if header.get(pe..pe.checked_add(4)?)? != b"PE\0\0" {
        return None;
    }
    // COFF header is 20 bytes after the signature; the optional header
    // follows, with AddressOfEntryPoint at +16 and SizeOfImage at +56.
    let time_date_stamp = u32_at(header, pe + 8)?;
    let entry_point_rva = u32_at(header, pe + 24 + 16)?;
    let size_of_image = u32_at(header, pe + 24 + 56)?;
    SIGNATURES
        .iter()
        .find(|s| {
            s.time_date_stamp == time_date_stamp
                && s.size_of_image == size_of_image
                && s.entry_point_rva == entry_point_rva
        })
        .map(|s| s.binary)
}

/// Identify the binary this process is running, reading the image header at
/// `GAME_IMAGE_BASE`. `None` if nothing readable there matches.
#[cfg(target_os = "windows")]
pub(crate) fn detect_current() -> Option<GameBinary> {
    use windows::Win32::System::Memory::{
        VirtualQuery, MEMORY_BASIC_INFORMATION, MEM_COMMIT, PAGE_NOACCESS,
    };

    unsafe {
        let mut info = MEMORY_BASIC_INFORMATION::default();
        let got = VirtualQuery(
            Some(GAME_IMAGE_BASE as *const core::ffi::c_void),
            &mut info,
            std::mem::size_of::<MEMORY_BASIC_INFORMATION>(),
        );
        if got == 0
            || info.State != MEM_COMMIT
            || info.Protect == PAGE_NOACCESS
            || info.RegionSize < HEADER_LEN
        {
            return None;
        }
        identify(std::slice::from_raw_parts(
            GAME_IMAGE_BASE as *const u8,
            HEADER_LEN,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A header with the given PE fields, laid out like a real one
    /// (e_lfanew = 0x80).
    fn header(stamp: u32, entry: u32, size: u32) -> Vec<u8> {
        let mut h = vec![0u8; HEADER_LEN];
        h[..2].copy_from_slice(b"MZ");
        h[0x3c..0x40].copy_from_slice(&0x80u32.to_le_bytes());
        h[0x80..0x84].copy_from_slice(b"PE\0\0");
        h[0x80 + 8..0x80 + 12].copy_from_slice(&stamp.to_le_bytes());
        h[0x80 + 24 + 16..0x80 + 24 + 20].copy_from_slice(&entry.to_le_bytes());
        h[0x80 + 24 + 56..0x80 + 24 + 60].copy_from_slice(&size.to_le_bytes());
        h
    }

    // Literals below were read from the shipped executables, not derived
    // from SIGNATURES.
    #[test]
    fn terran_exe_is_smac() {
        let h = header(0x3d590cc5, 0x2616bd, 0x60f000);
        assert_eq!(identify(&h), Some(GameBinary::Smac));
    }

    #[test]
    fn terranx_exe_is_alien_crossfire() {
        let h = header(0x385e91ff, 0x246c9d, 0x60c000);
        assert_eq!(identify(&h), Some(GameBinary::AlienCrossfire));
    }

    #[test]
    fn another_exe_is_unknown() {
        // wtp.exe, the launcher
        let h = header(0x6abe96b5, 0x14a0, 0xc000);
        assert_eq!(identify(&h), None);
    }

    #[test]
    fn stamp_alone_is_not_enough() {
        let h = header(0x385e91ff, 0x246c9d, 0x60f000);
        assert_eq!(identify(&h), None);
    }

    #[test]
    fn garbage_and_truncated_headers_are_unknown() {
        assert_eq!(identify(&[]), None);
        assert_eq!(identify(&[0u8; HEADER_LEN]), None);
        let h = header(0x3d590cc5, 0x2616bd, 0x60f000);
        assert_eq!(identify(&h[..0x90]), None);
        let mut no_pe = h.clone();
        no_pe[0x80] = b'X';
        assert_eq!(identify(&no_pe), None);
        // e_lfanew pointing past the end must not panic
        let mut far = h;
        far[0x3c..0x40].copy_from_slice(&0xffff_fff0u32.to_le_bytes());
        assert_eq!(identify(&far), None);
    }
}
