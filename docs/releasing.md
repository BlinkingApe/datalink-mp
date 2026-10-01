# Releasing

This document is for maintainers cutting a release. Players download releases
and follow the [README](../README.md); people building for themselves follow
[building.md](building.md).

Pushing a `v*` tag runs [`release.yml`](../.github/workflows/release.yml),
which builds every archive and opens a **draft** GitHub Release. CI never
publishes; a maintainer does, by hand, after the release gate
([ADR-0002](adr/0002-releases-versioning-pipeline-trust.md), section 4). The
bytes players get always come from CI, never from a local build.

You need the [GitHub CLI](https://cli.github.com/) (`gh`), logged in to an
account that can push to the repository.

## 1. Set the version

The tag must match the workspace version in the root `Cargo.toml`
(`[workspace.package] version`). Bump it by the rule in ADR-0002 section 1
(pre-1.0: a changed IPC or Peer protocol version is a minor bump, anything
else a patch), let `cargo build` update `Cargo.lock`, commit both and push
`main`.

## 2. Cut a release candidate

```bash
git tag v0.1.0-rc.1
git push origin v0.1.0-rc.1
```

- The tag is `v` + the workspace version, optionally followed by `-rc.N`.
  Anything else (`-rc`, `-beta`, `-rc.1-fix`) fails the run's first job and
  no draft appears. `scripts/release-version.sh <tag>` runs the same check
  locally.
- An RC's binaries already report the Release version it becomes (`0.1.0`, not
  `0.1.0-rc.1`), in the page and in Windows file properties. That is what
  lets the gated RC's archives become the release (section 4).
- Number RCs from 1 and never reuse a number: a new defect means a new
  `-rc.N` and a fresh gate.

## 3. What the run produces, and where

Watch the run under the repository's **Actions** tab, or with
`gh run watch`. When it finishes, the draft is on the **Releases** page (only
people with push access see drafts), marked pre-release for an `-rc.N` tag:

| Asset | What |
|---|---|
| `datalink-mp-<version>-windows-x86_64.zip` | Windows Helper, DLL, README, licences |
| `datalink-mp-<version>-linux-x86_64.tar.gz` | static musl Linux Helper, the same DLL, README, licences |
| `datalink-mp-<version>-macos-aarch64.zip` | Apple silicon Helper, the same DLL, README, licences; untested |
| `SHA256SUMS` | checksums of the archives |
| `attestation.sigstore.json` | the archives' build provenance, for offline checks |

- Archive names carry the **version, not the tag**, so an RC and its final
  release have identically named assets. That is what makes
  `gh release upload --clobber` replace them in section 4.
- The attestations are also stored by GitHub: they're listed on the run's
  summary page and under the repository's **Attestations** page
  (`/attestations`), and `gh attestation verify` finds them there. Verify
  downloaded archives with:

  ```bash
  gh release download v0.1.0-rc.1 -D rc1
  cd rc1
  sha256sum -c SHA256SUMS
  for a in *.zip *.tar.gz; do gh attestation verify "$a" -R BlinkingApe/datalink-mp; done
  ```

- The DLL step retries with `--allow-multiple-definition` if CI's mingw ever
  reports a `multiple definition of '_Unwind_Resume'` clash (see
  [building.md](building.md)). The run then shows a warning annotation; the
  retry is a fallback, not the expected path.
- All three archives hold the DLL built once by the Ubuntu job; the release
  job fails if their `dplayx.dll` hashes differ. So the macOS job waits for
  the Ubuntu job, and a run takes about as long as both back to back.
- The macOS archive is built on `macos-15` (arm64, free for public
  repositories; the `-large`/`-xlarge` runners are billed). Its log shows
  `codesign --verify` on the Helper; if the linker's ad-hoc signature didn't
  survive stripping, the job re-signs ad-hoc and verifies again, with a
  notice annotation. That only satisfies Gatekeeper's basic checks: players
  still go through Open Anyway (README).
- The macOS archive is packed with `ditto -c -k`, never `zip` or
  `upload-artifact` alone, which drop the executable bit. The job extracts
  it again with `ditto -x -k` and fails if the Helper isn't executable. To
  check by hand on a Mac, extract with Archive Utility or `ditto -x -k`, not
  `unzip`.
- The draft notes say macOS is built in CI and untested. Keep that wording
  until someone has run the gate on a Mac.
- The Ubuntu job checks the binaries before packing them: neither Windows
  binary may import a mingw runtime DLL, both must carry VERSIONINFO for the
  version, and the Linux Helper must be static. A failure there means the
  toolchain changed, not that the code did.

## 4. After the gate

Run the gate (ADR-0002 section 4) on one RC's archives. Then:

- Tag the gated RC's commit `v<version>` and push it. Its run builds new,
  different bytes (builds aren't reproducible) and opens its own draft.
- Replace that draft's archives and `SHA256SUMS` with the gated RC's, so the
  tested bytes are the ones that ship. The RC's attestations stay valid: they
  bind the archives' digests, not the tag.

  ```bash
  gh release download v0.1.0-rc.1 -D gated
  gh release upload v0.1.0 gated/* --clobber
  ```

- The draft's notes still name the `v<version>` run as the archives' source.
  Change that line to the gated RC's tag and run, which built the bytes and
  holds their attestations.
- Add the VirusTotal report links to the notes, then publish by hand.

## Cleaning up test runs

A run that fails after the draft was created, or a test tag you no longer
want, leaves a draft behind. Re-running the release job fails while a
release for the tag exists. Delete the draft and its tag together:

```bash
gh release delete v0.1.0-rc.1 --cleanup-tag --yes
git tag -d v0.1.0-rc.1
```

A tag whose version check failed has no draft; delete it with
`git push --delete origin <tag>` and `git tag -d <tag>`.
