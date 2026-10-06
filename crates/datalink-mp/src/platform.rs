//! Platform: the Helper's dealings with the machine it runs on.
//!
//! That is the Game folder self-check, and the browser opener, which differs
//! per operating system. The binary passes the opener into the library as a
//! function, so tests can replace it.

use crate::BrowserOpener;
use serde::Serialize;
use std::path::Path;
use tracing::warn;

/// The DLL's file name.
const DLL_FILE_NAME: &str = "dplayx.dll";

/// The file names of the game executables the Helper supports: Thinker's,
/// PRACX's and WTP's. One of them is enough.
const GAME_EXES: [&str; 3] = ["thinker.exe", "terran_PRACX.exe", "wtp.exe"];

/// What the Game folder self-check found.
#[derive(Debug, Clone, Serialize)]
pub struct SelfCheck {
    /// The folder that was checked: the one the Helper is running from
    pub folder: String,
    /// Whether the DLL is in the folder
    pub dll_found: bool,
    /// The file name of the game executable in the folder, if there is one
    pub game_exe: Option<String>,
    /// Whether this is a Game folder: it holds the DLL and a game executable
    pub passed: bool,
}

/// The Game folder self-check: is the Helper sitting next to the DLL and the
/// game? Only reads the folder's list of files.
///
/// An empty path stands for a folder that is not known, and fails the check.
pub(crate) fn check_game_folder(folder: &Path) -> SelfCheck {
    let file_names = file_names_in(folder);
    // Without regard to case: Windows does not care how a file name is
    // capitalised, so installs differ, and under Wine the difference shows.
    let find = |wanted: &str| file_names.iter().find(|name| name.eq_ignore_ascii_case(wanted));

    let dll_found = find(DLL_FILE_NAME).is_some();
    let game_exe = GAME_EXES.iter().find_map(|exe| find(exe)).cloned();
    SelfCheck {
        folder: folder.display().to_string(),
        dll_found,
        passed: dll_found && game_exe.is_some(),
        game_exe,
    }
}

/// The names of what is in `folder`. A folder that cannot be read holds
/// nothing the Helper can see.
fn file_names_in(folder: &Path) -> Vec<String> {
    // An empty path is not even tried: taken as a relative path, it would
    // name the working directory.
    if folder.as_os_str().is_empty() {
        return Vec::new();
    }
    let Ok(entries) = std::fs::read_dir(folder) else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter_map(|entry| entry.file_name().into_string().ok())
        .collect()
}

/// The real browser opener for this operating system.
///
/// Failing to open the browser is never an error: the Helper keeps running and
/// the launch URL is printed for the player to open themselves. The opener
/// therefore only answers whether it started the browser, and logs a failure
/// without the URL, which carries the token.
pub fn system_browser_opener() -> BrowserOpener {
    Box::new(|url: &str| match open_in_browser(url) {
        Ok(()) => true,
        Err(e) => {
            warn!("Could not open the browser: {e}");
            false
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
