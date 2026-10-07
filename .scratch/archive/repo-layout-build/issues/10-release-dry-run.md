# 10: Release dry run (HITL)

**What to build:** Proof that the real release pipeline still produces the same archives, on the layout branch, without releasing anything.

**Blocked by:** 09

**Status:** done

Spec: [../../repo-layout/spec.md](../../repo-layout/spec.md), and the decision tickets it cites.

## Acceptance criteria

- [x] Maintainer confirms pushing a throwaway pre-release tag (for example `v0.0.0-layout-test`) from the layout branch
- [x] Draft Release appears; maintainer compares its Windows, Linux and macOS file lists to the last real release; any difference fails and the layout is not merged unless it is a trivial workflow fix
- [x] Throwaway tag and draft Release are deleted

## Comments

2026-10-07. The tag isn't `v0.0.0-layout-test`. The workflow's first job (`scripts/release-version.sh`) only accepts `v<workspace version>` or `v<workspace version>-rc.N`, so that tag would have failed before building anything. The maintainer approved `v0.1.1-rc.99` instead, an obvious throwaway that can't collide with a real RC.

- Pushed annotated `v0.1.1-rc.99` on `layout` at 911cbc8. Run [37609101671](https://github.com/BlinkingApe/datalink-mp/actions/runs/37609101671): all four jobs green (version check, DLL + Windows + Linux, macOS, draft).
- The draft pre-release appeared with the same five assets as the published `v0.1.1`: `datalink-mp-0.1.1-{linux-x86_64.tar.gz,macos-aarch64.zip,windows-x86_64.zip}`, `SHA256SUMS`, `attestation.sigstore.json`. `sha256sum -c` passed on both.
- Archive contents, both releases downloaded and listed: identical. Each of Windows, Linux and macOS holds the Helper (`datalink-mp.exe` or `datalink-mp`), `dplayx.dll`, `datalink-mp-README.txt` and `datalink-mp-LICENSE-{APACHE,FONTS,MIT}.txt`. The shipped `datalink-mp-README.txt` is byte-identical to v0.1.1's, which shows `packaging/README.txt` ships under the old name. The draft's notes still open with `docs/release-notes/0.1.1.md`.
- The maintainer delegated the comparison when approving the dry run. This record is the evidence for them to check.
- Deleted the draft Release, the remote tag and the local tag. `gh release list` shows only `v0.1.1` and `v0.1.0`.
