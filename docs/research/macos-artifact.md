# Research: shipping an untested macOS (aarch64) artifact

Researched 2026-09-29. Scope: build `smac-helper` for `aarch64-apple-darwin` on
GitHub Actions, and package it in a `.zip` with the 32-bit `dplayx.dll`, a
README, licences and `SHA256SUMS`. The artifact is unsigned and nobody on the
project has run it on a Mac. The goal is that users need no terminal apart from
one Gatekeeper approval.

Each claim links to its source. Anything marked **unverified** comes from
secondary reports or inference and needs checking on a real Mac.

## TL;DR

1. **Runner:** use `runs-on: macos-15` (arm64, M1). You can also use `macos-26`,
   which `macos-latest` points to now. `macos-14` is being retired.
   Standard runners cost nothing on public repos. The lockfile uses `ring`
   rather than `aws-lc-rs`, so no cmake or nasm is needed. iroh's own releases
   build `aarch64-apple-darwin` on macOS 15 runners with no special steps.
2. **DLL:** build `dplayx.dll` once in the Linux job and download that artifact
   into the macOS packaging job. Do not rebuild it with Homebrew mingw-w64.
   Builds from different toolchains will not be byte-identical. Even two builds
   on the same machine differ, because the PE header timestamp and the
   `$CARGO_HOME` paths in panic strings change. This does not matter for
   compatibility, since the protocol version string decides that. It does
   matter for "one tested DLL, one hash" across all three archives.
3. **Gatekeeper (macOS 15 and later):** Control-click → Open **no longer
   bypasses** Gatekeeper. The only path without a terminal is to try opening
   the file, then go to System Settings → Privacy & Security → **Open Anyway**
   (within about an hour), enter the login password, and open it again.
   Arm64 code must carry at least an ad-hoc signature. The Apple linker adds
   one automatically. CI should run `codesign --verify` on the binary and
   re-sign ad hoc if the check fails.
4. **"No terminal" is not literally achievable with a bare binary.** When you
   double-click a Mach-O file in Finder, Finder opens it **in Terminal.app**.
   A Terminal window appears and shows the helper's output, and closing that
   window quits the helper. Users do not have to type anything. Reword the
   promise as "no typing commands" (see §3.4). Running from Terminal has one
   upside: macOS 15's Local Network privacy check automatically allows
   command-line tools started from Terminal.
5. **Browser:** the `open` crate runs `/usr/bin/open -- <url>`, which opens the
   default browser from a process started by Terminal. **Unverified** on
   hardware, but this is the standard LaunchServices path.

---

## 1. Building `smac-helper` for aarch64-apple-darwin on GitHub Actions

### Runner labels (as of 2026-09)

According to the [runner-images README](https://github.com/actions/runner-images)
and [GitHub-hosted runners reference](https://docs.github.com/en/actions/reference/runners/github-hosted-runners):

| Label | OS | Arch | Spec | Status |
|---|---|---|---|---|
| `macos-latest`, `macos-26` | macOS 26 (Tahoe) | arm64 (M1) | 3 CPU, 7 GB | GA; `macos-latest` points here |
| `macos-15` | macOS 15 (Sequoia) | arm64 (M1) | 3 CPU, 7 GB | GA |
| `macos-14` | macOS 14 | arm64 | — | **Deprecated.** Deprecation starts 2026-07-06 and support ends 2026-11-02 ([runner-images#13518](https://github.com/actions/runner-images/issues/13518)) |
| `macos-15-intel`, `macos-26-intel` | 15 / 26 | x86_64 | 4 CPU, 14 GB | GA |

The runner-images README says the `-latest` migration "is gradual and happens
over 1-2 months" and recommends pinning an explicit version. **Recommendation:**
pin `macos-15`. A binary built on 15 runs on 26. The Rust default deployment
target for `aarch64-apple-darwin` is macOS 11.0
([rustc platform support: apple-darwin](https://doc.rust-lang.org/rustc/platform-support/apple-darwin.html)),
so the SDK version on the runner does not limit which macOS versions the binary
supports. Moving up later is a one-line change.

Preinstalled tooling on the images, from
[macos-15-arm64-Readme](https://github.com/actions/runner-images/blob/main/images/macos/macos-15-arm64-Readme.md)
and the macos-26 equivalent: Rust 1.98.1, Cargo, and Rustup 1.29.0.
**mingw-w64 is not preinstalled** on the macOS images, and it is not listed on
the Ubuntu 24.04 image either.

### Cost

- "Use of the standard GitHub-hosted runners is free and unlimited on public
  repositories"
  ([GitHub-hosted runners](https://docs.github.com/en/actions/reference/runners/github-hosted-runners)).
- The same page on [billing](https://docs.github.com/en/billing/concepts/product-billing/github-actions)
  lists macOS 3/4-core at $0.062/min. That rate only applies to private repos
  once the free quota is used up. The fork is public, so the cost is **$0**.
  Larger runners (`-xlarge`, `-large`) are billed even on public repos, so do
  not use them.

### iroh-specific build concerns

- iroh builds on macOS without special handling. Its own
  [release workflow](https://github.com/n0-computer/iroh/blob/main/.github/workflows/release.yml)
  builds `aarch64-apple-darwin` with a plain
  `cargo build --locked --target aarch64-apple-darwin` on a macOS 15 runner.
  It has no macOS-specific steps and does no codesigning. Its
  [tests workflow](https://github.com/n0-computer/iroh/blob/main/.github/workflows/tests.yaml)
  runs the test suite on self-hosted macOS ARM64 runners. The only
  platform-specific setup there is cmake and nasm for `aws-lc-sys` on
  **Windows**.
- This workspace's `Cargo.lock` contains `ring 0.17.14` under `rustls 0.23` and
  **no `aws-lc-sys`**, so no cmake or nasm is needed. `ring` compiles its C and
  assembly with the Xcode clang that is already on the runner.
- The other native-ish iroh dependencies in the lockfile are `netwatch 0.19`,
  `netdev 0.45` and `portmapper 0.19`. They use macOS system frameworks, which
  iroh's own macOS CI builds, so nothing extra is needed.
- `[profile.release]` sets `strip = true`. From Rust 1.98 onwards, rustc strips
  Mach-O files with its bundled `rust-objcopy`
  ([rustc `link.rs`](https://github.com/rust-lang/rust/blob/main/compiler/rustc_codegen_ssa/src/back/link.rs):
  `if sess.target.is_like_darwin { let stripcmd = "rust-objcopy"; … "--strip-all" }`).
  llvm-objcopy regenerates `LC_CODE_SIGNATURE` when it modifies a signed
  Mach-O ([D111164](https://reviews.llvm.org/D111164)), so the linker's ad-hoc
  signature should survive. Apple warns that tools which modify a binary after
  linking "might need" a manual `codesign` afterwards
  ([Big Sur 11.0.1 Universal Apps release notes, Code Signing](https://developer.apple.com/documentation/macos-release-notes/macos-big-sur-11_0_1-universal-apps-release-notes)).
  To be safe, CI should run
  `codesign --verify --verbose smac-helper || codesign --force --sign - smac-helper`.
  A reported pitfall: if a rustc wrapper strips `DYLD_FALLBACK_LIBRARY_PATH`,
  `rust-objcopy` cannot find `libLLVM.dylib` and stripping fails with only a
  warning ([kache#1326](https://github.com/kunobi-ninja/kache/issues/1326),
  secondary source). This only matters if sccache or a similar wrapper is added.
- Intel Macs cannot run an arm64-only binary. If Intel support is wanted later,
  `x86_64-apple-darwin` can be cross-built on the same arm64 runner
  (`rustup target add x86_64-apple-darwin`) and combined with `lipo -create`.
  This is out of scope for now. Say "Apple Silicon only" in the README.

### Suggested job shape (sketch, not tested)

```yaml
jobs:
  dll:
    runs-on: ubuntu-24.04
    steps:
      - uses: actions/checkout@v5
      - run: sudo apt-get update && sudo apt-get install -y gcc-mingw-w64-i686
      - run: rustup target add i686-pc-windows-gnu
      - run: cargo build --locked --release --target i686-pc-windows-gnu -p dplayx
      - uses: actions/upload-artifact@v7
        with: { name: dplayx-dll, path: target/i686-pc-windows-gnu/release/dplayx.dll }

  macos:
    needs: dll
    runs-on: macos-15
    steps:
      - uses: actions/checkout@v5
      - run: rustup target add aarch64-apple-darwin   # already the host; harmless
      - run: cargo build --locked --release -p smac-helper --target aarch64-apple-darwin
      - uses: actions/download-artifact@v5
        with: { name: dplayx-dll, path: dist/ }
      - run: |
          B=target/aarch64-apple-darwin/release/smac-helper
          codesign --verify --verbose "$B" || codesign --force --sign - "$B"
          file "$B"                      # expect: Mach-O 64-bit executable arm64
          cp "$B" README.md LICENSE-MIT LICENSE-APACHE dist/
          chmod +x dist/smac-helper
          (cd dist && shasum -a 256 * > SHA256SUMS)
          ditto -c -k --keepParent dist smac-iroh-macos-arm64-UNTESTED.zip
      - uses: actions/upload-artifact@v7
        with: { path: smac-iroh-macos-arm64-UNTESTED.zip, archive: false }
```

Build the release `.zip` yourself, as above, rather than relying on
`upload-artifact`'s automatic zipping. Its
[README](https://github.com/actions/upload-artifact) says: "File permissions are
not maintained during zipped artifact upload. All directories will have `755`
and all files will have `644`." Without the executable bit, double-clicking the
binary does nothing useful. `ditto -c -k` and `zip` both keep Unix modes.

Pick one checksum convention and use it consistently. Either put `SHA256SUMS`
inside the zip covering its members, or attach it next to the zip as a release
asset. For people checking the download, a hash of the `.zip` itself published
on the release page is the more useful of the two.

---

## 2. Where the macOS archive's `dplayx.dll` should come from

**Recommendation: reuse the Linux job's artifact.**

Reasons:

- **The same DLL on every platform.** Windows, Linux and macOS users then all
  run the one DLL that has actually been tested under Wine. The Windows helper
  plan (ADR-0001 §5) compares the installed DLL's hash with the embedded one.
  A single canonical DLL keeps that comparison meaningful across platforms and
  makes support simpler ("what's your DLL's SHA256?").
- **The toolchains differ.** Homebrew `mingw-w64` is currently 14.0.0 with
  GCC 16.2 ([formulae.brew.sh/api/formula/mingw-w64.json](https://formulae.brew.sh/api/formula/mingw-w64.json),
  with arm64 Sequoia and Tahoe bottles). Ubuntu 24.04's `gcc-mingw-w64-i686` is
  an older GCC and mingw-w64 runtime. Rust links the mingw CRT start-up objects
  and `libmingwex` from the linker's sysroot, so different toolchains give
  different bytes. For this 1999 32-bit target, the Linux-built DLL is the one
  with a test history.
- **Byte-identity is not achievable anyway, and it does not matter
  functionally.** The DLL in the main checkout
  (`target/i686-pc-windows-gnu/release/dplayx.dll`, built on Fedora with
  mingw GCC 15.1) contains:
  - a PE link timestamp (`objdump -p`: `Time/Date Tue Sep 29 16:33:51 2026`),
    which changes on every link;
  - absolute build-machine paths in panic-location strings, such as
    `/home/jct/.cargo/registry/src/index.crates.io-…/postcard-1.1.3/src/varint.rs`.
    59 strings contain `/home/`. A runner build would contain `/home/runner/…`
    or `/Users/runner/…` instead.

  So two builds are never byte-identical without extra work
  (`--remap-path-prefix`, `-Wl,--no-insert-timestamp`). Wire compatibility
  between helpers and DLLs depends on the protocol version string, not on the
  DLL's bytes. What matters is that each release ships **one** DLL whose hash
  is published, and reusing the Linux artifact gives exactly that.
- **Speed and simplicity.** You skip `brew install mingw-w64`, which is a large
  bottle, on the slower 3-core macOS runner. The macOS job then only needs the
  host Rust toolchain.

Keep `brew install mingw-w64` in the **build-from-source** docs (§5), because
Mac users building locally need it. It is already mentioned in
`.cargo/config.toml` and `docs/INSTALL.md`.

The imports of the current DLL are `kernel32`, `ntdll`, `msvcrt`, `WS2_32`,
`bcryptprimitives` and `api-ms-win-core-synch-l1-2-0`. There is no
`libgcc_s_*.dll` or `libwinpthread` dependency, so the DLL is self-contained
and nothing else from mingw needs to go in the zip.

---

## 3. Gatekeeper on macOS 15 Sequoia / 26 Tahoe (and 27)

### 3.1 Control-click → Open no longer works

Apple Developer News, 2024-08-06,
["Updates to runtime protection in macOS Sequoia"](https://developer.apple.com/news/?id=saqachfa):

> In macOS Sequoia, users will no longer be able to Control-click to override
> Gatekeeper when opening software that isn't signed correctly or notarized.
> They'll need to visit System Settings > Privacy & Security to review security
> information for software before allowing it to run.

The README must **not** tell users to right-click → Open. That advice is only
correct for macOS 14 and older.

### 3.2 The supported path: Open Anyway

Apple's Mac User Guide,
["Open a Mac app from an unknown developer"](https://support.apple.com/guide/mac-help/open-a-mac-app-from-an-unknown-developer-mh40616/mac)
(the version selector covers Catalina through macOS 27):

1. Apple menu → System Settings → **Privacy & Security** in the sidebar.
2. Go to **Security**, then click **Open**. In the UI this is the
   "“smac-helper” was blocked…" row with an **Open Anyway** button.
3. Click **Open Anyway**.
4. Enter your login password, then click OK.

"This button is available for about an hour after you try to open the app."
The user therefore has to try opening the file **first**, and that attempt is
blocked. Apple's companion article
[Safely open apps on your Mac (102445)](https://support.apple.com/en-us/102445)
adds: "The warning prompt reappears and … you can click Open … [it] is now saved
as an exception to your security settings, and you can open it in the future by
double-clicking it."

**Unverified:** the exact dialog text a quarantined, ad-hoc-signed CLI binary
triggers on 15 and 26. Reports suggest a "“smac-helper” Not Opened / Apple
could not verify … is free of malware" alert with **Done** and **Move to
Trash**. The README should describe the flow without quoting dialog text word
for word.

### 3.3 Signature requirement on Apple Silicon

[Big Sur 11.0.1 Universal Apps release notes → Code Signing](https://developer.apple.com/documentation/macos-release-notes/macos-big-sur-11_0_1-universal-apps-release-notes):

> New in macOS 11 on Macs with Apple silicon … the operating system enforces
> that any executable must be signed before it's allowed to run. There isn't a
> specific identity requirement for this signature: a simple ad-hoc signature
> is sufficient.

The same notes say the toolchain "will now automatically sign your executables"
at link time. They also say: "given that these signatures do not bear any valid
identity, binaries signed this way cannot pass through Gatekeeper." So:

- rustc on macOS links through Apple's `ld` (via `cc`), so the binary arrives
  **ad-hoc linker-signed** without any extra step.
- The ad-hoc signature is **required** for the binary to run at all on arm64.
  Without it the kernel kills the process (`Killed: 9`). It does **not** help
  with Gatekeeper, so users still need Open Anyway.
- CI must check that the signature survived `strip` (see §1).

### 3.4 What double-clicking a bare Mach-O actually does

- Finder does not execute a command-line tool directly. It **hands it to
  Terminal.app as a document**. Terminal opens a new window with a shell and
  runs the tool in it. Apple DTS (Quinn "The Eskimo!") on the
  [Developer Forums, thread 127403](https://developer.apple.com/forums/thread/127403),
  Dec 2019:

  > the Finder passes the script or tool to Terminal as a document and that
  > opens a window (and associated shell) in which to run that document. This
  > triggers Gatekeeper's document logic. That logic hasn't been updated to
  > understand notarisation … and thus Gatekeeper always fails the open
  > document request.

  In March 2020 he added: "if you double click a tool, it's blocked by
  Gatekeeper in the same way as a script." Being unsigned is therefore not the
  only thing blocking a double-clicked quarantined tool. Double-clicking goes
  through a Gatekeeper path that blocks even notarised tools, so there is no
  way to avoid the Open Anyway step except removing the quarantine.
- The file needs the executable bit, or Finder will not treat it as runnable
  ([Scripting OS X, "Launching Scripts from Finder"](https://scriptingosx.com/2022/04/launching-scripts-2-launching-scripts-from-finder/),
  secondary). Hence the zip-permission note in §1.
- **What users see, in order (inferred; must be confirmed on hardware):**
  1. They double-click `smac-helper`, and a Gatekeeper alert blocks it.
     Scripting OS X says a quarantined script may instead fail inside Terminal
     with `operation not permitted`. Either could happen, so the README must
     cover both.
  2. System Settings → Privacy & Security → Open Anyway → password.
  3. They double-click again and confirm Open. **A Terminal window opens**
     showing the helper's log, and the helper opens the browser.
  4. The Terminal window's working directory is the user's home folder, **not**
     the folder the binary is in. The helper must find its files with
     `std::env::current_exe()`, never the working directory.
  5. Closing the Terminal window (or pressing ⌘Q in Terminal) stops the helper.
     This is the macOS counterpart of the Windows "console window is the quit
     control" rule in ADR-0001. When the helper exits by itself, the window
     stays open and shows `[Process completed]`.
- **So the promise needs rewording.** A Terminal window will appear. Suggested
  README wording: *"No commands to type. The first time, macOS will block the
  helper. Approve it once in System Settings → Privacy & Security → Open
  Anyway. After that, double-click `smac-helper`. A Terminal window opens and
  your browser shows the helper. Leave the Terminal window open while you
  play. Closing it stops the helper."*
- **The Terminal launch helps with networking.** macOS 15 added Local Network
  privacy. Apple's
  [TN3179: Understanding local network privacy](https://developer.apple.com/documentation/technotes/tn3179-understanding-local-network-privacy)
  says macOS automatically allows local network access for "command-line tools
  run from Terminal or over SSH, including any child processes they spawn".
  Other programs are blocked until the user grants access. TN3179 also says the
  privilege is tracked by code signature, which is a problem for ad-hoc
  signing. A `.app` bundle wrapper would hide Terminal, but it would bring
  back: a Local Network prompt, whose grant would likely be lost on every
  release because the ad-hoc identity changes; App Translocation for
  quarantined apps (the binary would run from a random read-only path, which
  breaks "find the game folder next to me"); and still the same Open Anyway
  step. **Keep the bare binary for the untested release.** Revisit a `.app`
  only if the project gets a Developer ID. Loopback traffic between the helper
  and the DLL on 127.0.0.1 is not "local network" in any case.
- **Terminal fallback** (for the README's troubleshooting section only; it
  breaks the no-terminal promise):
  `xattr -dr com.apple.quarantine "<folder you extracted to>"`. After that,
  Gatekeeper does not assess the files.

### 3.5 Other first-run prompts

- macOS's Application Firewall is off on most Macs. If a user has it on, they
  may get an "accept incoming network connections?" prompt for an unsigned
  binary (**unverified**; Apple's
  [firewall settings page](https://support.apple.com/guide/mac-help/change-firewall-settings-on-mac-mh11783/mac)
  only covers auto-allowing *signed* software). List it under
  troubleshooting.
- **Whisky has been archived and is unmaintained** since 2025-05-11. Its
  [repository](https://github.com/Whisky-App/Whisky) says: "Whisky is no longer
  actively maintained. Apps and games may break at any time." The macOS
  README, `docs/INSTALL.md` and `scripts/launch-whisky.sh` all assume Whisky.
  The untested release should say it targets any Wine setup on macOS, naming
  Whisky and CrossOver as examples, and should not promise Whisky
  compatibility.
- Bottles live under `~/Library/Containers/…`, and `~/Library` is hidden in
  Finder. To reach the game folder without a terminal, users need Finder's
  **Go → Go to Folder…** (⇧⌘G) or their Wine front-end's "open C: drive"
  action. The README should give the ⇧⌘G route with a path template.

---

## 4. Opening the default browser

- `open` crate, macOS backend
  ([docs.rs source, `src/macos.rs`](https://docs.rs/crate/open/latest/source/src/macos.rs)):
  it runs `/usr/bin/open -- <path-or-url>`, or `open -a <app> -- …` with
  `with_command`. `/usr/bin/open` asks LaunchServices to open the URL in the
  user's default browser.
- The helper runs as a normal user process inside the user's GUI session,
  started by Terminal, so LaunchServices is available and `open` works as it
  would from a shell. No entitlement or permission is needed to open an
  `http://127.0.0.1:…` URL. **Unverified on hardware**, but it is the same
  mechanism as typing `open https://…` in Terminal.
- As ADR-0001 already requires, the helper should also print the URL. On macOS
  it appears in the Terminal window, and ⌘-double-click on a URL there opens
  it. That is a real fallback if `open` fails.
- The macOS backend adds no extra dependencies, so the ADR's "tiny crate" rule
  holds.

---

## 5. What the macOS build-from-source instructions need to say

A locally built binary is **not quarantined**: only downloads get
`com.apple.quarantine`. The linker also ad-hoc signs it. So building from
source avoids the Gatekeeper step completely. That is worth saying for
technical users who don't want to trust an unsigned download.

Prerequisites and steps:

1. **Xcode Command Line Tools**: run `xcode-select --install`. This provides
   `cc`, `ld`, `codesign` and the SDK. rustc finds the SDK through
   `xcrun --sdk macosx --show-sdk-path`
   ([rustc apple-darwin docs](https://doc.rust-lang.org/rustc/platform-support/apple-darwin.html)).
2. **Rust via rustup** (https://rustup.rs). The host target
   `aarch64-apple-darwin` is installed by default. Also add
   `rustup target add i686-pc-windows-gnu` for the DLL.
3. **Homebrew** and `brew install mingw-w64`. This provides
   `i686-w64-mingw32-gcc`, which `.cargo/config.toml` names as the linker for
   `i686-pc-windows-gnu`. Homebrew ships arm64 bottles for Sequoia and Tahoe
   ([formula](https://formulae.brew.sh/api/formula/mingw-w64.json)). On Apple
   Silicon, Homebrew installs to `/opt/homebrew/bin`, which must be on `PATH`;
   the Homebrew installer prints the line to add.
4. Build:
   ```bash
   cargo build --release -p smac-helper
   cargo build --release --target i686-pc-windows-gnu -p dplayx
   ```
   The outputs are `target/release/smac-helper` (arm64 Mach-O, ad-hoc signed)
   and `target/i686-pc-windows-gnu/release/dplayx.dll`.
5. Copy both into the game folder inside the bottle, or run the helper from
   `target/release`, whichever the helper's DLL-discovery design supports.
6. The DLL override step (`dplayx` = native) is the same as today.
7. Remind builders that everyone in a game needs the **same release/protocol
   version**. A self-built helper talking to a CI-built helper works only if
   they come from the same tag.
8. Apple Silicon only. Intel Macs need `--target x86_64-apple-darwin`, which is
   untested.

---

## 6. Open items for someone with a Mac

These cannot be settled from documentation. Ask the first Mac user to report:

1. The exact first-run alert for the double-clicked, quarantined binary on 15.x
   and on 26.x, and whether it is an alert or `operation not permitted` inside
   Terminal.
2. Whether Open Anyway, then double-click, then Open actually launches it in
   Terminal. Also whether macOS asks again on the second launch, and after an
   update replaces the file.
3. `codesign -dv smac-helper` output, to confirm the ad-hoc signature survived
   `strip`.
4. Whether the browser opens automatically.
5. Whether the helper's DLL-to-helper TCP link on 127.0.0.1 works from Wine
   running under Rosetta. It should, since loopback is not subject to Local
   Network privacy, but it has never been tested.
6. Any Local Network or firewall prompts during a real iroh session.

## Sources

- GitHub: [GitHub-hosted runners reference](https://docs.github.com/en/actions/reference/runners/github-hosted-runners), [Actions billing](https://docs.github.com/en/billing/concepts/product-billing/github-actions), [runner-images README](https://github.com/actions/runner-images), [macos-15-arm64 image readme](https://github.com/actions/runner-images/blob/main/images/macos/macos-15-arm64-Readme.md), [macOS 14 deprecation](https://github.com/actions/runner-images/issues/13518), [actions/upload-artifact](https://github.com/actions/upload-artifact)
- Apple: [Updates to runtime protection in macOS Sequoia](https://developer.apple.com/news/?id=saqachfa), [Open a Mac app from an unknown developer](https://support.apple.com/guide/mac-help/open-a-mac-app-from-an-unknown-developer-mh40616/mac), [Safely open apps on your Mac](https://support.apple.com/en-us/102445), [Big Sur 11.0.1 Universal Apps release notes](https://developer.apple.com/documentation/macos-release-notes/macos-big-sur-11_0_1-universal-apps-release-notes), [TN3179 Local network privacy](https://developer.apple.com/documentation/technotes/tn3179-understanding-local-network-privacy), [DTS on double-clicked tools (forums 127403)](https://developer.apple.com/forums/thread/127403)
- Rust: [rustc apple-darwin platform support](https://doc.rust-lang.org/rustc/platform-support/apple-darwin.html), [rustc `link.rs` strip logic](https://github.com/rust-lang/rust/blob/main/compiler/rustc_codegen_ssa/src/back/link.rs)
- LLVM: [D111164 Regenerate LC_CODE_SIGNATURE in llvm-objcopy](https://reviews.llvm.org/D111164)
- iroh: [release.yml](https://github.com/n0-computer/iroh/blob/main/.github/workflows/release.yml), [tests.yaml](https://github.com/n0-computer/iroh/blob/main/.github/workflows/tests.yaml)
- Other: [Homebrew mingw-w64 formula](https://formulae.brew.sh/api/formula/mingw-w64.json), [open crate macOS source](https://docs.rs/crate/open/latest/source/src/macos.rs), [Whisky repository (archived)](https://github.com/Whisky-App/Whisky); secondary: [Scripting OS X](https://scriptingosx.com/2022/04/launching-scripts-2-launching-scripts-from-finder/), [kache#1326](https://github.com/kunobi-ninja/kache/issues/1326)
