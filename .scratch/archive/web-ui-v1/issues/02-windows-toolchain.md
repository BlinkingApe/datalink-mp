# Windows helper toolchain: cross-compile -gnu locally vs -msvc in CI?

Type: research
Status: resolved
Blocked by:

## Question

Which Windows target should the x86_64 Helper use, and how is it built?

- Can `x86_64-pc-windows-gnu` be cross-compiled from Linux (Fedora/RHEL-family host with mingw-w64) for this workspace, including Iroh's dependency tree (ring/aws-lc, etc.)? What are the known gotchas?
- `-gnu` vs `-msvc`: differences in binary size, runtime DLL dependencies (e.g. libgcc/winpthread static linking), antivirus reputation, and how easy each is on GitHub Actions `windows-latest` / `ubuntu-latest`.
- The 32-bit DLL (`i686-pc-windows-gnu`) is already built locally with mingw; confirm the CI recipe for it.
- A recommended target and a concrete command set for both a local experiment and CI.

## Research

Findings: branch `research/windows-toolchain`, file `docs/research/windows-toolchain.md`.

## Comments

### Resolution (2026-09-29)

**Use `x86_64-pc-windows-gnu` for the Helper.** Full findings: branch `research/windows-toolchain` (commit `a142bcd`), `docs/research/windows-toolchain.md`, including local commands, an additive `release.yml` sketch, and an msvc fallback job.

- **Crypto:** the lockfile pulls only `ring`, with no aws-lc, so no cmake, NASM or clang is needed.
- **Local cross-build succeeds:** 6m23s cold, 7.16 MB PE32+ console exe. It imports only DLLs that ship with Windows; libgcc/winpthread are linked statically. Not yet run on real Windows.
- **The 32-bit DLL build fails as documented** on this host (rustc 1.98.1, mingw GCC 15.1): `multiple definition of _Unwind_Resume`, because `crates/dplayx/unwind_stubs.c` clashes with `libgcc_eh.a`.
  - The existing local DLL was built with `RUSTFLAGS=...--allow-multiple-definition`. That **replaces** the `.cargo/config.toml` rustflags, silently dropping `control-flow-guard=no` and `-lws2_32`. **The DLL you tested may not match a proper build.**
  - Workaround: `CARGO_TARGET_I686_PC_WINDOWS_GNU_RUSTFLAGS` adds to the config rustflags instead of replacing them (verified with `-v`).
  - Cleaner: delete the `unwind_stubs.c` build step from `build.rs`. Tested in a scratch copy: same size, no libgcc dependency. It's a production change and upstream-PR-worthy.
- **-gnu over -msvc:** the rustc docs support cross-compiling -gnu from Linux but not -msvc. -gnu uses the same toolchain as the DLL, so one `ubuntu-24.04` job builds the whole zip. No primary source says -msvc gets better AV reputation.
- **CI:** install `gcc-mingw-w64-i686 gcc-mingw-w64-x86-64` and add both rustup targets. Risk: Ubuntu 24.04's mingw (GCC 13.2) is older than what rustc tests against, so check it on the first run. Fallback: a Fedora container, or msvc with `+crt-static` on `windows-latest` for the Helper only.
