# 06: Startup: browser opening, UI port walk, readable startup failure

**What to build:** A player double-clicks the Helper and their browser opens the page, on every OS, with no command typed. If the usual UI port is taken the Helper picks the next free one. If the Helper cannot start its networking at all, the window stays open with a plain explanation.

Scope, from the spec's "Browser opening", "Command line" and "Decisions this spec adds" sections:

- **Browser opening.** The Platform module provides the real opener that the binary passes into the library.
  - Windows: the `webbrowser` crate or `ShellExecuteW` through `windows-sys`. The `open` crate must not be used, because it spawns a hidden PowerShell that antivirus products flag.
  - Linux: `xdg-open`.
  - macOS: `/usr/bin/open`.
  - A failure to open the browser is not an error; the URL is always printed.
- **UI port walk.** If the chosen UI port is taken, try the next nine (47700 to 47709 by default). If all ten are taken, exit with a clear message. The same walk applies to a port given by flag or environment variable. The printed launch URL and the opener both use the port actually bound.
- **Startup failure.** If the first Transport cannot be created in UI mode, the Helper prints the error in plain words and waits for Enter before exiting, so a double-clicked window does not vanish.

The macOS opener comes from documentation, not from a test on a Mac; keep it simple and say so in a comment.

**Blocked by:** 05 (UI mode tracer bullet)

**Status:** resolved

- [ ] Double-clicking the Helper on Linux opens the default browser at the launch URL
- [ ] The Windows build opens the browser without spawning PowerShell, and the `open` crate is not a dependency
- [ ] The macOS build uses `/usr/bin/open`
- [ ] When the opener fails, the Helper keeps running and the launch URL is still printed
- [ ] Test: with the default UI port held by the test, the Helper binds the next port and reports it in its handle and launch URL
- [ ] Test: with all ten ports in the range held, startup fails with a clear message
- [ ] The walk starts from a port given by `--ui-port` or `SMAC_UI_PORT`
- [ ] When the first Transport cannot be created in UI mode, a plain-words error is printed and the process waits for Enter before exiting
- [ ] `host` and `join` are unaffected: they open no browser and do not wait for Enter
