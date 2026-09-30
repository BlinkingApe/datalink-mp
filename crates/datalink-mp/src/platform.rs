//! Platform: the parts of the Helper that differ per operating system.
//!
//! Today that is the browser opener. The binary passes it into the library as
//! a function, so tests can replace it.

use crate::BrowserOpener;
use tracing::warn;

/// The real browser opener for this operating system.
///
/// Failing to open the browser is never an error: the Helper keeps running and
/// the launch URL is printed for the player to open themselves. The opener
/// therefore returns nothing, and logs a failure without the URL, which
/// carries the token.
pub fn system_browser_opener() -> BrowserOpener {
    Box::new(|url: &str| {
        if let Err(e) = open_in_browser(url) {
            warn!("Could not open the browser: {e}");
        }
    })
}

/// Windows: `ShellExecuteW` with the "open" verb on the URL, which is what the
/// default browser's registration handles.
///
/// Deliberately not the `open` crate: its 5.4.x releases spawn a hidden
/// `powershell.exe`, a pattern antivirus products flag.
#[cfg(windows)]
fn open_in_browser(url: &str) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use std::ptr::{null, null_mut};
    use windows_sys::Win32::UI::Shell::ShellExecuteW;
    use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

    fn wide(text: &str) -> Vec<u16> {
        std::ffi::OsStr::new(text).encode_wide().chain(Some(0)).collect()
    }

    // SAFETY: both strings are NUL-terminated and outlive the call; the
    // remaining arguments are documented to accept null.
    let result = unsafe {
        ShellExecuteW(
            null_mut(),
            wide("open").as_ptr(),
            wide(url).as_ptr(),
            null(),
            null(),
            SW_SHOWNORMAL,
        )
    };
    // ShellExecuteW reports success with a value above 32.
    if result as usize > 32 {
        Ok(())
    } else {
        Err(std::io::Error::other(format!(
            "ShellExecuteW failed with code {}",
            result as usize
        )))
    }
}

/// macOS: `/usr/bin/open <url>`.
///
/// This comes from Apple's documentation for `open`, not from a test on a Mac:
/// nobody has run it on one yet. Kept as simple as possible on purpose.
#[cfg(target_os = "macos")]
fn open_in_browser(url: &str) -> std::io::Result<()> {
    spawn_detached("/usr/bin/open", url)
}

/// Linux and the other Unixes: `xdg-open <url>`, found through `PATH`.
#[cfg(not(any(windows, target_os = "macos")))]
fn open_in_browser(url: &str) -> std::io::Result<()> {
    spawn_detached("xdg-open", url)
}

/// Start `program url` and return without waiting for it.
///
/// `xdg-open` can stay alive as long as the browser it started does, so the
/// Helper must not wait for it. A thread reaps the child when it ends, so it
/// does not linger as a zombie.
#[cfg(not(windows))]
fn spawn_detached(program: &str, url: &str) -> std::io::Result<()> {
    use std::process::{Command, Stdio};
    use tracing::debug;

    let mut child = Command::new(program)
        .arg(url)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    std::thread::spawn(move || match child.wait() {
        Ok(status) if !status.success() => debug!("The browser opener exited with {status}"),
        Ok(_) => {}
        Err(e) => debug!("Could not wait for the browser opener: {e}"),
    });
    Ok(())
}
