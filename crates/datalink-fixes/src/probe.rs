//! Non-invasive logging probes for SMAC debugging
//!
//! This module provides a way to inject logging probes at specific addresses
//! without altering game behavior. The probe logs a fixed message to file,
//! then executes the original code.
//!
//! ## Design
//!
//! - Trampoline-based hooking with pure x86 assembly
//! - Ring buffer for deferred logging (no Rust calls from trampoline)
//! - Background thread drains buffer to file
//! - Thread-safe via LOCK XADD for multi-writer support
//!
//! ## Usage
//!
//! ```ignore
//! init_probes()?;
//! let id = register_probe("HostGameStartHandler ENTERED");
//! unsafe {
//!     install_probe(0x543a70, 6, id, &[0x6a, 0xff, 0x68, 0x2b, 0xfb, 0x67])?;
//! }
//! ```
//!
//! Set `SMAC_PROBE_LOG=/path/to/probe.log` to enable logging.

use std::sync::atomic::{AtomicU32, Ordering};

#[cfg(target_os = "windows")]
use windows::Win32::System::Memory::{
    VirtualAlloc, VirtualProtect, MEM_COMMIT, MEM_RESERVE, PAGE_EXECUTE_READWRITE,
    PAGE_PROTECTION_FLAGS,
};

// ============================================================================
// Constants
// ============================================================================

/// Ring buffer size (must be power of 2)
const BUFFER_SIZE: usize = 4096;
const BUFFER_MASK: u32 = (BUFFER_SIZE - 1) as u32;

/// Maximum number of probes
const MAX_PROBES: usize = 256;

/// Size of trampoline allocation
const TRAMPOLINE_SIZE: usize = 64;

/// Minimum instruction length for hooking (need space for JMP rel32)
const MIN_HOOK_SIZE: usize = 5;

/// Environment variable for probe log file path
const PROBE_LOG_ENV: &str = "SMAC_PROBE_LOG";

// ============================================================================
// Global State
// ============================================================================

/// Ring buffer for probe IDs
static mut RING_BUFFER: [u8; BUFFER_SIZE] = [0; BUFFER_SIZE];

/// Write pointer (atomically incremented by trampolines)
static WRITE_PTR: AtomicU32 = AtomicU32::new(0);

/// Message table - initialized once at startup, never modified after
static mut PROBE_MESSAGES: [Option<&'static str>; MAX_PROBES] = [None; MAX_PROBES];

/// Number of registered probes
static mut PROBE_COUNT: u8 = 0;

/// Whether the probe system has been initialized
static mut INITIALIZED: bool = false;

// ============================================================================
// Public API
// ============================================================================

/// Initialize the probe system.
///
/// This spawns the background drain thread and sets up global state.
/// Call once at DLL load time.
///
/// IMPORTANT: Only spawns the drain thread if SMAC_PROBE_LOG is set.
/// Spawning threads during DllMain can cause issues with the loader lock.
pub fn init_probes() -> Result<(), &'static str> {
    unsafe {
        if INITIALIZED {
            return Ok(());
        }
        INITIALIZED = true;
    }

    // Only spawn drain thread if logging is enabled
    // Spawning threads during DllMain with loader lock held can crash Wine
    if std::env::var(PROBE_LOG_ENV).is_ok() {
        std::thread::spawn(drain_thread);
    }

    Ok(())
}

/// Register a probe message.
///
/// Returns a probe_id (0-255) that can be passed to install_probe.
/// Messages must be `&'static str`.
pub fn register_probe(message: &'static str) -> u8 {
    unsafe {
        let id = PROBE_COUNT;
        if (id as usize) < MAX_PROBES {
            PROBE_MESSAGES[id as usize] = Some(message);
            PROBE_COUNT = id + 1;
        }
        id
    }
}

/// Install a logging probe at the given address.
///
/// # Arguments
///
/// * `address` - The address to hook
/// * `instruction_len` - Number of bytes to relocate (must be >= 5, on instruction boundary)
/// * `probe_id` - The probe ID from register_probe
/// * `expected_bytes` - Expected bytes at address (for binary validation)
///
/// # Safety
///
/// - Caller must ensure `instruction_len` is on an instruction boundary
/// - Caller must ensure the address is executable code
/// - Must be called after init_probes()
#[cfg(target_os = "windows")]
pub unsafe fn install_probe(
    address: usize,
    instruction_len: usize,
    probe_id: u8,
    expected_bytes: &[u8],
) -> Result<(), &'static str> {
    if instruction_len < MIN_HOOK_SIZE {
        return Err("instruction_len must be >= 5");
    }

    if !INITIALIZED {
        return Err("probe system not initialized");
    }

    // Validate expected bytes
    let actual = std::slice::from_raw_parts(address as *const u8, expected_bytes.len());
    if actual != expected_bytes {
        return Err("binary validation failed - unexpected bytes at address");
    }

    // Allocate trampoline
    let trampoline = VirtualAlloc(
        None,
        TRAMPOLINE_SIZE,
        MEM_COMMIT | MEM_RESERVE,
        PAGE_EXECUTE_READWRITE,
    );
    if trampoline.is_null() {
        return Err("failed to allocate trampoline");
    }
    let trampoline = trampoline as *mut u8;

    // Copy original bytes
    let original_bytes = std::slice::from_raw_parts(address as *const u8, instruction_len);

    // Build trampoline code
    let return_addr = address + instruction_len;
    let code = build_trampoline(probe_id, original_bytes, trampoline as usize, return_addr);

    // Write trampoline
    std::ptr::copy_nonoverlapping(code.as_ptr(), trampoline, code.len());

    // Patch original site with JMP to trampoline
    let mut old_protect = PAGE_PROTECTION_FLAGS(0);
    if VirtualProtect(
        address as *const core::ffi::c_void,
        instruction_len,
        PAGE_EXECUTE_READWRITE,
        &mut old_protect,
    )
    .is_err()
    {
        return Err("VirtualProtect failed");
    }

    // Write JMP rel32
    let jmp_target = (trampoline as isize) - (address as isize) - 5;
    let site = address as *mut u8;
    *site = 0xE9; // JMP rel32
    std::ptr::copy_nonoverlapping(
        &(jmp_target as i32) as *const i32 as *const u8,
        site.add(1),
        4,
    );

    // NOP padding if instruction_len > 5
    for i in 5..instruction_len {
        *site.add(i) = 0x90; // NOP
    }

    // Restore protection
    let mut dummy = PAGE_PROTECTION_FLAGS(0);
    let _ = VirtualProtect(
        address as *const core::ffi::c_void,
        instruction_len,
        old_protect,
        &mut dummy,
    );

    Ok(())
}

/// Stub for non-Windows builds
#[cfg(not(target_os = "windows"))]
pub unsafe fn install_probe(
    _address: usize,
    _instruction_len: usize,
    _probe_id: u8,
    _expected_bytes: &[u8],
) -> Result<(), &'static str> {
    Ok(())
}

// ============================================================================
// Trampoline Builder
// ============================================================================

/// Build trampoline code for a probe.
///
/// Layout:
/// - PUSHAD, PUSHFD (save registers)
/// - Atomic ring buffer write (LOCK XADD)
/// - POPFD, POPAD (restore registers)
/// - Original instructions
/// - JMP back to return address
fn build_trampoline(
    probe_id: u8,
    original_bytes: &[u8],
    trampoline_addr: usize,
    return_addr: usize,
) -> Vec<u8> {
    let mut code = Vec::with_capacity(TRAMPOLINE_SIZE);

    // Get addresses of globals (using raw pointers to avoid static_mut_refs warning)
    let write_ptr_addr = WRITE_PTR.as_ptr() as usize;
    let buffer_addr = std::ptr::addr_of!(RING_BUFFER) as usize;

    // PUSHAD - save all general purpose registers
    code.push(0x60);

    // PUSHFD - save flags
    code.push(0x9C);

    // MOV EAX, 1 (increment value)
    code.push(0xB8);
    code.extend_from_slice(&1u32.to_le_bytes());

    // MOV EDX, write_ptr_addr (address of atomic counter)
    code.push(0xBA);
    code.extend_from_slice(&(write_ptr_addr as u32).to_le_bytes());

    // LOCK XADD [EDX], EAX (atomic fetch-and-add, old value in EAX)
    code.push(0xF0); // LOCK prefix
    code.push(0x0F);
    code.push(0xC1);
    code.push(0x02); // [EDX]

    // AND EAX, BUFFER_MASK (wrap to buffer size)
    code.push(0x25);
    code.extend_from_slice(&BUFFER_MASK.to_le_bytes());

    // MOV EDX, buffer_addr (buffer base address)
    code.push(0xBA);
    code.extend_from_slice(&(buffer_addr as u32).to_le_bytes());

    // MOV BYTE [EDX+EAX], probe_id (store probe_id at claimed slot)
    code.push(0xC6);
    code.push(0x04);
    code.push(0x02); // [EDX+EAX]
    code.push(probe_id);

    // POPFD - restore flags
    code.push(0x9D);

    // POPAD - restore registers
    code.push(0x61);

    // Copy original instructions
    code.extend_from_slice(original_bytes);

    // JMP rel32 back to return address
    let jmp_offset = code.len();
    code.push(0xE9);
    // Calculate relative offset: target - (current_address + 5)
    let current_addr = trampoline_addr + jmp_offset;
    let rel = (return_addr as i32) - (current_addr as i32) - 5;
    code.extend_from_slice(&rel.to_le_bytes());

    code
}

// ============================================================================
// Background Drain Thread
// ============================================================================

/// Format a SystemTime as RFC3339 UTC with millisecond precision
/// (e.g. `2026-07-14T03:22:49.110Z`), matching the DLL's tracing log format
/// so probe lines can be correlated with DLL log lines.
///
/// Timestamps are taken at DRAIN time, so they lag the actual probe firing
/// by up to one drain interval (10ms).
fn format_timestamp(t: std::time::SystemTime) -> String {
    let dur = t
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = dur.as_secs() as i64;
    let millis = dur.subsec_millis();

    let days = secs.div_euclid(86_400);
    let secs_of_day = secs.rem_euclid(86_400);
    let (hh, mm, ss) = (secs_of_day / 3600, (secs_of_day / 60) % 60, secs_of_day % 60);

    // Civil-from-days (Howard Hinnant's algorithm), valid for our date range
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };

    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:03}Z",
        y, m, d, hh, mm, ss, millis
    )
}

fn drain_thread() {
    use std::fs::OpenOptions;
    use std::io::Write;
    use std::time::Duration;

    // Open log file from env var, or exit if not set
    let log_file = match std::env::var(PROBE_LOG_ENV) {
        Ok(path) => match OpenOptions::new().create(true).append(true).open(&path) {
            Ok(f) => Some(f),
            Err(e) => {
                eprintln!("datalink-fixes: failed to open probe log {}: {}", path, e);
                None
            }
        },
        Err(_) => None, // No env var = probes disabled
    };

    let Some(mut file) = log_file else {
        return; // No log file, exit thread
    };

    let mut local_read = 0u32;

    loop {
        std::thread::sleep(Duration::from_millis(10));

        // Relaxed load - we just want to eventually see new entries
        let write_pos = WRITE_PTR.load(Ordering::Relaxed);

        // One timestamp per drain batch: entries drained together fired within
        // the same 10ms window, and a single clock read keeps the hot loop cheap.
        let batch_ts = if local_read != write_pos {
            Some(format_timestamp(std::time::SystemTime::now()))
        } else {
            None
        };

        unsafe {
            while local_read != write_pos {
                let idx = (local_read & BUFFER_MASK) as usize;
                // Use raw pointer access to avoid static_mut_refs warning
                let buffer_ptr = std::ptr::addr_of!(RING_BUFFER) as *const u8;
                let probe_id = *buffer_ptr.add(idx) as usize;
                local_read = local_read.wrapping_add(1);

                // Use raw pointer access to avoid static_mut_refs warning
                let messages_ptr = std::ptr::addr_of!(PROBE_MESSAGES) as *const [Option<&'static str>; MAX_PROBES];
                if let Some(msg) = (*messages_ptr).get(probe_id).and_then(|m| *m) {
                    let ts = batch_ts.as_deref().unwrap_or("");
                    let _ = writeln!(file, "{} [PROBE] {}", ts, msg);
                }
            }
        }

        // Flush to ensure data is written
        let _ = file.flush();
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_probe() {
        unsafe {
            PROBE_COUNT = 0;
            PROBE_MESSAGES = [None; MAX_PROBES];
        }

        let id1 = register_probe("test message 1");
        let id2 = register_probe("test message 2");

        assert_eq!(id1, 0);
        assert_eq!(id2, 1);

        unsafe {
            assert_eq!(PROBE_MESSAGES[0], Some("test message 1"));
            assert_eq!(PROBE_MESSAGES[1], Some("test message 2"));
        }
    }

    #[test]
    fn test_format_timestamp() {
        use std::time::{Duration, UNIX_EPOCH};
        // 2026-07-14T03:22:49.110Z == epoch 1783999369.110
        let t = UNIX_EPOCH + Duration::from_millis(1_783_999_369_110);
        assert_eq!(format_timestamp(t), "2026-07-14T03:22:49.110Z");
        // Epoch itself
        assert_eq!(format_timestamp(UNIX_EPOCH), "1970-01-01T00:00:00.000Z");
        // Leap-day
        let t = UNIX_EPOCH + Duration::from_secs(1_582_934_400); // 2020-02-29T00:00:00Z
        assert_eq!(format_timestamp(t), "2020-02-29T00:00:00.000Z");
    }

    #[test]
    fn test_trampoline_size() {
        // Verify trampoline fits in allocation
        let code = build_trampoline(0, &[0x90; 10], 0x1000, 0x100A);
        assert!(code.len() <= TRAMPOLINE_SIZE);
    }
}
