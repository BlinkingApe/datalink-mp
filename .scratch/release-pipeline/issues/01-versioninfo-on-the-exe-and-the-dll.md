# 01: VERSIONINFO on the exe and the DLL

**What to build:** A player who right-clicks `datalink-mp.exe` or `dplayx.dll` → Properties → Details sees the Release version without starting anything. Both Windows binaries carry a VERSIONINFO resource whose version comes from the workspace version, so it can never drift from the Release version the page shows.

Scope, from the spec's "Implementation Decisions" (VERSIONINFO) and "Decisions this spec adds" (ordering, tool):

- Use the `embed-resource` crate, which drives the mingw `windres` the cross-builds already install. Resources are compiled only for Windows targets; native Linux and macOS builds stay untouched.
- The Helper has no `build.rs` yet; the DLL's already exists and keeps its `.def` handling.
- **The DLL is a `cdylib`.** `embed-resource`'s plain compile call links the resource into binaries only, so the DLL needs the variant that reaches every artifact (check the crate's docs for the current name). Prove the resource is actually in the built DLL, not just that the build passed.
- Leave the `unwind_stubs` step alone; removing it belongs to ticket 02.

Verify through seam 2: the local cross-builds in `docs/building.md`, then inspect both binaries' resources from Linux.

**Blocked by:** None (can start immediately)

**Status:** resolved

- [ ] The cross-built `datalink-mp.exe` (`x86_64-pc-windows-gnu`) contains a VERSIONINFO resource with the workspace version as its file and product version
- [ ] The cross-built `dplayx.dll` (`i686-pc-windows-gnu`) contains the same
- [ ] The version is read from the workspace version at build time; nothing hard-codes `0.1.0`
- [ ] The product name in both resources is `datalink-mp`
- [ ] A native Linux build of the workspace builds and tests exactly as before
- [ ] The DLL still exports what `dplayx.def` lists (compare the export table before and after)
