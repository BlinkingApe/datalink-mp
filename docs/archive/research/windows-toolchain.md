# Research: Windows toolchain for the x86_64 Helper

- **Date:** 2026-09-29
- **Question:** Which Windows target should the x86_64 `smac-helper.exe` use, and how is it built (locally for experiments, in GitHub Actions for tagged releases)? Also: confirm the CI recipe for the 32-bit `dplayx.dll`.
- **Context:** [ADR 0001](../../adr/0001-web-ui-frontend.md) section 7 leaves `x86_64-pc-windows-msvc` vs `-gnu` open. Already decided: the Windows release is a `.zip` holding `smac-helper.exe` and `dplayx.dll` side by side (the DLL is not embedded). Experiments are cross-built on the Linux dev host. Tagged releases are built by GitHub Actions. CI changes should be additive so the fork stays upstream-friendly.

## TL;DR

1. **Use `x86_64-pc-windows-gnu`.** It cross-compiles cleanly from this Linux host today. Measured result: a 7.2 MB PE32+ console exe whose imports are all Windows system DLLs (no `libgcc_s_seh-1.dll`, no `libwinpthread-1.dll`). The same mingw toolchain already builds the DLL, so one `ubuntu-latest` job can produce the whole Windows zip.
2. **Iroh's crypto is ring-only.** `Cargo.lock` contains `ring 0.17.14` and no `aws-lc-rs`/`aws-lc-sys`, so we don't need cmake, NASM or clang. The crates that use C/asm are just `ring`, `blake3` and our own `dplayx`, and all three build with plain `x86_64-w64-mingw32-gcc`.
3. **Problem found: the DLL build as documented fails on this host.** `cargo build --release --target i686-pc-windows-gnu -p dplayx` fails at link time with `multiple definition of '_Unwind_Resume'`, because `crates/dplayx/unwind_stubs.c` now clashes with `libgcc_eh.a`. The DLL in the main checkout was built with `RUSTFLAGS="-C link-arg=-Wl,--allow-multiple-definition"`, according to shell history. CI needs a workaround or a small fix (see [the DLL section](#32-bit-dll-recipe-and-a-link-failure-found-while-testing)).
4. **Keep MSVC as the fallback.** Build it natively on `windows-latest`. Don't use cargo-xwin for release builds, since rustc says cross-compiling to MSVC from non-Windows hosts "may be possible but is not supported".

## Local evidence

Environment: RHEL/Fedora-family el10, rustc 1.98.1 (`48a229cea 2026-09-01`), rustup targets `i686-pc-windows-gnu` and `x86_64-pc-windows-gnu` (the second was added with `rustup target add`), Fedora `mingw64-gcc-15.1.1-1.el10` / `mingw32-gcc-15.1.1-1.el10` (GCC 15.1.1). No clang, lld-link or cargo-xwin is installed.

### Crypto/native dependency tree (from `Cargo.lock` / `cargo tree`)

- `grep -c aws-lc Cargo.lock` returns `0`. No aws-lc-rs.
- `ring 0.17.14` comes in via `noq-proto` (iroh's QUIC stack) and `rustls 0.23.42` (whose lockfile deps list `ring`), which is used by hickory-resolver / hyper-rustls / iroh-relay.
- Packages whose deps include `cc`: `blake3`, `dplayx`, `generator`, `iana-time-zone-haiku`, `ring`. Only `ring` and `blake3` matter for the helper on Windows.
- `blake3 1.8.5`'s `build.rs` compiles `c/blake3_*_x86-64_windows_gnu.S` for gnu targets. It only compiles `*_windows_msvc.asm` (MASM) when the target is msvc *and* the C compiler is `cl`/`cl.exe` (`use_msvc_asm()`, lines 54-77, 214-222 of the vendored crate). With cargo-xwin (which uses clang-cl) it would also take the `.S` path.
- ring's [BUILDING.md](https://github.com/briansmith/ring/blob/main/BUILDING.md): "For Windows x86 and x86-64, the packaged crate contains precompiled object files for the assembly language modules so no macro assembler" is needed. NASM is only needed when building ring from git. For MSVC targets it requires "Build Tools for Visual Studio 2022 ... Desktop development with C++".

### x86_64-pc-windows-gnu helper build (this host)

```sh
rustup target add x86_64-pc-windows-gnu
cargo build --release --target x86_64-pc-windows-gnu -p smac-helper
```

- Result: **success**, with no extra flags. The existing `.cargo/config.toml` already sets `linker = "x86_64-w64-mingw32-gcc"`. A cold build took 6m23s wall-clock (release profile: `lto = true`, `opt-level = "z"`, `strip = true`, `panic = "abort"`).
- `target/x86_64-pc-windows-gnu/release/smac-helper.exe`: **7,162,880 bytes**, `PE32+ executable (console) x86-64`.
- Imports (`x86_64-w64-mingw32-objdump -p ... | grep 'DLL Name'`), deduplicated: `kernel32.dll`, `ntdll.dll`, `advapi32.dll`, `ws2_32.dll`, `iphlpapi.dll`, `bcrypt.dll`, `bcryptprimitives.dll`, `combase.dll`, `ole32.dll`, `oleaut32.dll`, `api-ms-win-core-synch-l1-2-0.dll`, `msvcrt.dll`. All of these ship with Windows. **No mingw runtime DLLs**: rustc's windows-gnu link line uses `-lgcc_eh -l:libpthread.a` (see the i686 link line below), so libgcc's unwinder and winpthreads are linked statically.
- Not tested: running the exe (Wine isn't installed here). The ADR already reports that the helper and DLL have been tested on real Windows.

### 32-bit DLL recipe, and a link failure found while testing

The ADR/`.cargo/config.toml` recipe, run in a clean worktree:

```sh
cargo build --release --target i686-pc-windows-gnu -p dplayx
```

**Fails** on this host:

```
ld: .../libgcc_eh.a(unwind-dw2.o): in function `Unwind_Resume':
    .../libgcc/unwind.inc:231: multiple definition of `_Unwind_Resume';
    .../libunwind_stubs.a(unwind_stubs.o):unwind_stubs.c:(.text$_Unwind_Resume+0x0): first defined here
collect2: error: ld returned 1 exit status
```

Cause: `crates/dplayx/build.rs` compiles `unwind_stubs.c`, which defines `_Unwind_Resume` on the assumption that "libgcc_eh provides other unwind symbols but not this one". With rustc 1.98.1 + GCC 15.1 mingw, rustc links `-lgcc_eh` (visible in the link line) and pulls in `unwind-dw2.o`, which also defines it. The DLL in the main checkout (built 16:33 today) came from `RUSTFLAGS="-C link-arg=-Wl,--allow-multiple-definition" cargo build ...`, according to shell history.

Three options, all verified locally:

| Option | Result |
|---|---|
| `RUSTFLAGS="-C link-arg=-Wl,--allow-multiple-definition"` | Builds, **but** Cargo treats `RUSTFLAGS` as mutually exclusive with `target.<triple>.rustflags` ([Cargo config: build.rustflags](https://doc.rust-lang.org/cargo/reference/config.html#buildrustflags)), so this silently drops `-C control-flow-guard=no` and `-lws2_32` from `.cargo/config.toml`. Avoid it. |
| `CARGO_TARGET_I686_PC_WINDOWS_GNU_RUSTFLAGS="-C link-arg=-Wl,--allow-multiple-definition"` | Builds. `cargo build -v` shows it is **joined** with the config-file flags (`-lws2_32`, `control-flow-guard=no` are still present), per "Arrays will be joined together" ([Cargo config: hierarchical structure](https://doc.rust-lang.org/cargo/reference/config.html#hierarchical-structure)). 951,808-byte DLL. This is the right CI workaround if the code isn't touched. |
| Remove the `cc::Build ... unwind_stubs` call from `crates/dplayx/build.rs` (tested in a scratch copy only, not committed) | Builds cleanly with the plain command. Same 951,808-byte DLL. Imports are only `kernel32`, `ntdll`, `ws2_32`, `msvcrt`, `bcryptprimitives`, `api-ms-win-core-synch-l1-2-0`, with no libgcc DLL. This is the cleanest and most upstreamable fix. It's a production-code change, so it is left for a separate commit/PR. |

Not verified: whether Ubuntu 24.04's mingw (GCC 13.2) hits the same conflict. The `-lgcc_eh` comes from rustc's target spec, not from the distro, so assume it does until CI proves otherwise.

## -gnu vs -msvc

| | `x86_64-pc-windows-gnu` | `x86_64-pc-windows-msvc` |
|---|---|---|
| Tier | Tier 1 with host tools ([rustc windows-gnu](https://doc.rust-lang.org/rustc/platform-support/windows-gnu.html)) | Tier 1 with host tools |
| Cross-compile from Linux | Supported: "Rust does ship a pre-compiled std library for those targets ... one can easily compile and cross-compile for those targets from other hosts if C proper toolchain is installed" ([rustc windows-gnu](https://doc.rust-lang.org/rustc/platform-support/windows-gnu.html)). **Verified here.** | "Cross-compilation from a non-Windows host to a `*-windows-msvc` target *may* be possible but is **not supported**" ([rustc windows-msvc](https://doc.rust-lang.org/rustc/platform-support/windows-msvc.html)). Possible with cargo-xwin (below). Not tested here: no clang on this host. |
| Toolchain the Rust project tests with | GNU Binutils 2.44, GCC 14.2, mingw-w64 12.0.0. "Using older tools (especially Binutils) may not work properly" ([rustc windows-gnu](https://doc.rust-lang.org/rustc/platform-support/windows-gnu.html)) | VS 2017 minimum, latest VS "highly recommended" |
| Runtime DLL deps | Measured: none beyond system DLLs and `msvcrt.dll` (libgcc_eh and winpthreads are linked statically) | Links the VC runtime (`vcruntime140.dll`) dynamically unless `-C target-feature=+crt-static` ([Reference: static and dynamic C runtimes](https://doc.rust-lang.org/reference/linkage.html#static-and-dynamic-c-runtimes)). For a no-installer zip you'd set `+crt-static`. |
| Binary size | 7.16 MB measured | Not measured. Expected to be the same order of magnitude (same Rust code, same LTO/opt-level z). Not a deciding factor. |
| One toolchain for exe + DLL | Yes, since the DLL is `i686-pc-windows-gnu` anyway | No: you'd need mingw for the DLL plus MSVC for the exe |
| GitHub Actions | `ubuntu-latest` (Ubuntu 24.04) has Rust 1.98.1 / rustup 1.29.1 but **no mingw**, so apt-install it ([Ubuntu2404-Readme](https://github.com/actions/runner-images/blob/main/images/ubuntu/Ubuntu2404-Readme.md)). Noble ships `gcc-mingw-w64-x86-64` 13.2.0, `mingw-w64` 11.0.1 and `binutils-mingw-w64` 2.41.90 ([packages.ubuntu.com](https://packages.ubuntu.com/noble/gcc-mingw-w64-x86-64)), which is older than rustc's tested set (see risk below). | `windows-latest` = Windows Server 2025 + **Visual Studio 2026** image ([runner-images README](https://github.com/actions/runner-images#available-images), [Windows2025-VS2026-Readme](https://github.com/actions/runner-images/blob/main/images/windows/Windows2025-VS2026-Readme.md)), with Rust 1.98.1 and MSVC build tools preinstalled, so it builds natively with zero setup. MSYS2 is at `C:\msys64` but not on PATH. |
| AV / SmartScreen reputation | No primary source found. It's widely reported (anecdotally) that mingw-built executables trip heuristic AV more often than MSVC ones. **Unverified.** | Same caveat. Unsigned exes get SmartScreen warnings either way. ADR 0001 already scopes code signing out and plans README warnings + checksums, which applies to both targets. Not having `dplayx.dll` embedded in the exe probably helps more than the choice of toolchain. |

### cargo-xwin (MSVC from Linux)

From the [cargo-xwin README](https://github.com/rust-cross/cargo-xwin): install with `cargo install --locked cargo-xwin`. It needs `clang` and `rustup component add llvm-tools` ("A full LLVM installation is recommended to avoid possible issues"). Build with `cargo xwin build --target x86_64-pc-windows-msvc`. It downloads the MSVC CRT and Windows SDK, and "By using this software you are consented to accept the license at https://go.microsoft.com/fwlink/?LinkId=2086102". This works for ring/blake3 in principle, but it's officially unsupported by rustc, adds a Microsoft-license download step, and needs clang (not installed on this host, and installing it was out of scope). For this project it adds nothing over -gnu.

## Recommendation

**Target `x86_64-pc-windows-gnu` for the helper.** Reasons:

- It is verified to cross-build on the dev host with no extra configuration beyond what `.cargo/config.toml` already has.
- It shares one toolchain family (mingw-w64) with the mandatory 32-bit `i686-pc-windows-gnu` DLL, so a single `ubuntu-latest` job produces the whole Windows zip. Local experiments and CI use the same recipe.
- The resulting exe has no redistributable-runtime dependency (measured).
- The "Windows host" CI job stays optional, which keeps the upstream diff small.

**Risk and fallback:** Ubuntu 24.04's mingw (GCC 13.2 / binutils 2.41) is older than what rustc tests against (GCC 14.2 / binutils 2.44). If the CI build misbehaves:
- (a) pin `runs-on: ubuntu-24.04` and try a newer container, e.g. `fedora:latest` with `mingw64-gcc mingw32-gcc`, which matches the dev host; or
- (b) build only the helper on `windows-latest` as `x86_64-pc-windows-msvc` with `RUSTFLAGS=-C target-feature=+crt-static`, and keep the DLL on Ubuntu.

Either way the zip layout doesn't change.

**Separately (needs a decision, not done here):** fix the `_Unwind_Resume` clash by dropping the `unwind_stubs.c` build step in `crates/dplayx/build.rs`. That is the upstreamable fix. Until then, use the `CARGO_TARGET_I686_PC_WINDOWS_GNU_RUSTFLAGS` workaround, not `RUSTFLAGS`.

## Commands

### Local experiment (this host)

```sh
rustup target add i686-pc-windows-gnu x86_64-pc-windows-gnu
# Fedora/RHEL: mingw64-gcc and mingw32-gcc (already installed here)

# DLL (workaround until unwind_stubs is removed)
CARGO_TARGET_I686_PC_WINDOWS_GNU_RUSTFLAGS="-C link-arg=-Wl,--allow-multiple-definition" \
  cargo build --release --target i686-pc-windows-gnu -p dplayx
# Helper
cargo build --release --target x86_64-pc-windows-gnu -p smac-helper

# Inspect
x86_64-w64-mingw32-objdump -p target/x86_64-pc-windows-gnu/release/smac-helper.exe | grep 'DLL Name'
i686-w64-mingw32-objdump  -p target/i686-pc-windows-gnu/release/dplayx.dll        | grep 'DLL Name'

# Package
mkdir -p dist/smac-iroh-windows-x86_64
cp target/x86_64-pc-windows-gnu/release/smac-helper.exe \
   target/i686-pc-windows-gnu/release/dplayx.dll \
   README.md LICENSE-MIT LICENSE-APACHE dist/smac-iroh-windows-x86_64/
(cd dist && zip -r smac-iroh-windows-x86_64.zip smac-iroh-windows-x86_64 && sha256sum *.zip > SHA256SUMS)
```

### CI job sketch (new, additive file, e.g. `.github/workflows/release.yml`)

```yaml
name: release
on:
  push:
    tags: ["v*"]
  workflow_dispatch:

jobs:
  windows-zip:
    runs-on: ubuntu-24.04          # pin rather than ubuntu-latest for reproducibility
    steps:
      - uses: actions/checkout@v4
      - name: Install mingw-w64
        run: |
          sudo apt-get update
          sudo apt-get install -y --no-install-recommends \
            gcc-mingw-w64-i686 gcc-mingw-w64-x86-64 zip
      - name: Rust targets
        run: rustup target add i686-pc-windows-gnu x86_64-pc-windows-gnu
      - uses: Swatinem/rust-cache@v2
      - name: Build dplayx.dll (32-bit)
        env:
          # Remove once crates/dplayx no longer builds unwind_stubs.c
          CARGO_TARGET_I686_PC_WINDOWS_GNU_RUSTFLAGS: "-C link-arg=-Wl,--allow-multiple-definition"
        run: cargo build --release --target i686-pc-windows-gnu -p dplayx
      - name: Build smac-helper.exe (x86_64)
        run: cargo build --release --target x86_64-pc-windows-gnu -p smac-helper
      - name: Check no mingw runtime DLL imports
        run: |
          ! x86_64-w64-mingw32-objdump -p target/x86_64-pc-windows-gnu/release/smac-helper.exe \
              | grep -iE 'DLL Name: (libgcc|libwinpthread|libstdc)'
          ! i686-w64-mingw32-objdump -p target/i686-pc-windows-gnu/release/dplayx.dll \
              | grep -iE 'DLL Name: (libgcc|libwinpthread|libstdc)'
      - name: Package
        run: |
          d=smac-iroh-${GITHUB_REF_NAME}-windows-x86_64
          mkdir -p dist/$d
          cp target/x86_64-pc-windows-gnu/release/smac-helper.exe \
             target/i686-pc-windows-gnu/release/dplayx.dll \
             README.md LICENSE-MIT LICENSE-APACHE dist/$d/
          cd dist && zip -r $d.zip $d && sha256sum $d.zip > $d.zip.sha256
      - uses: actions/upload-artifact@v4
        with:
          name: windows-x86_64
          path: dist/*.zip*
```

Fallback helper job (only if the Ubuntu toolchain misbehaves), then combine with the DLL artifact in a packaging job:

```yaml
  helper-msvc:
    runs-on: windows-latest
    env:
      RUSTFLAGS: "-C target-feature=+crt-static"   # no vcruntime140.dll dependency
    steps:
      - uses: actions/checkout@v4
      - run: cargo build --release --target x86_64-pc-windows-msvc -p smac-helper
      - uses: actions/upload-artifact@v4
        with: { name: helper-msvc, path: target/x86_64-pc-windows-msvc/release/smac-helper.exe }
```

## Open items

- Run the CI sketch once to confirm Ubuntu 24.04's mingw 13.2 builds both artifacts. Check whether it also needs the `_Unwind_Resume` workaround.
- Smoke-test the -gnu `smac-helper.exe` on real Windows (or Wine) before the first tagged release.
- Decide whether to remove `unwind_stubs.c` (upstream PR candidate).

## Sources

- rustc book, windows-gnu: https://doc.rust-lang.org/rustc/platform-support/windows-gnu.html
- rustc book, windows-msvc: https://doc.rust-lang.org/rustc/platform-support/windows-msvc.html
- Rust Reference, linkage / crt-static: https://doc.rust-lang.org/reference/linkage.html#static-and-dynamic-c-runtimes
- Cargo config reference (rustflags precedence, array merging): https://doc.rust-lang.org/cargo/reference/config.html
- ring BUILDING.md: https://github.com/briansmith/ring/blob/main/BUILDING.md
- blake3 1.8.5 `build.rs` (vendored in `~/.cargo/registry`)
- cargo-xwin README: https://github.com/rust-cross/cargo-xwin
- GitHub runner images: https://github.com/actions/runner-images (README label table; `Ubuntu2404-Readme.md` image 20260920.314.1; `Windows2025-VS2026-Readme.md` image 20260922.246.2)
- Ubuntu noble packages: https://packages.ubuntu.com/noble/gcc-mingw-w64-x86-64, https://packages.ubuntu.com/noble/mingw-w64, https://packages.ubuntu.com/noble/binutils-mingw-w64-i686
- Local measurements: this worktree, 2026-09-29 (commands above)
