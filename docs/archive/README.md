# Archive

Finished material, kept for the record. Nothing here is current: the live docs
are under `docs/players/`, `docs/contributors/` and `docs/maintainers/`. The root
`.ignore` hides this folder and `.scratch/archive/` from ripgrep-based search,
so look here on purpose (`rg --no-ignore --hidden`, or name the path).

Archived files keep the paths and names they were written with, so their links
may point at files that have since moved. The tables below map old paths to
new ones.

To get an old file back, check it out from the tag in the last column:
`git show pre-layout:<old path>` prints it, and
`git checkout pre-layout -- <old path>` restores it. `pre-layout` is the
commit before the 2026-10 repo layout change.

## Evidence docs in `docs/archive/`

| Old path | New path | Recover from |
|---|---|---|
| `docs/research/av-risk.md` | [`docs/archive/research/av-risk.md`](research/av-risk.md) | `pre-layout`; branch `research/av-risk` is tag `archive/research-av-risk` |
| `docs/research/iroh-connection-stats.md` | [`docs/archive/research/iroh-connection-stats.md`](research/iroh-connection-stats.md) | `pre-layout` |
| `docs/research/macos-artifact.md` | [`docs/archive/research/macos-artifact.md`](research/macos-artifact.md) | `pre-layout`; branch `research/macos-artifact` is tag `archive/research-macos-artifact` |
| `docs/research/repo-layout-conventions.md` | [`docs/archive/research/repo-layout-conventions.md`](research/repo-layout-conventions.md) | `pre-layout` |
| `docs/research/smac-jackal-turn-sync.md` | [`docs/archive/research/smac-jackal-turn-sync.md`](research/smac-jackal-turn-sync.md) | `pre-layout` |
| `docs/research/transport-restart.md` | [`docs/archive/research/transport-restart.md`](research/transport-restart.md) | `pre-layout` |
| `docs/research/windows-toolchain.md` | [`docs/archive/research/windows-toolchain.md`](research/windows-toolchain.md) | `pre-layout`; branch `research/windows-toolchain` is tag `archive/research-windows-toolchain` |
| `docs/datalink-mp-0.1.0-release-checklist.html` | [`docs/archive/0.1.0-release-checklist.html`](0.1.0-release-checklist.html) | `pre-layout` |
| `tools/0001-wtp-through-the-ages.html` | [`docs/archive/wtp-through-the-ages.html`](wtp-through-the-ages.html) | `pre-layout` |

## Finished efforts in `.scratch/archive/`

| Old path | New path | Recover from |
|---|---|---|
| `.scratch/release-pipeline/` | [`.scratch/archive/release-pipeline/`](../../.scratch/archive/release-pipeline/) | `pre-layout` |
| `.scratch/ui-polish-0.1.1/` | [`.scratch/archive/ui-polish-0.1.1/`](../../.scratch/archive/ui-polish-0.1.1/) | `pre-layout` |
| (archived earlier) | [`.scratch/archive/web-ui-v1/`](../../.scratch/archive/web-ui-v1/) | unchanged by the layout change |
| (archived earlier) | [`.scratch/archive/helper-web-ui/`](../../.scratch/archive/helper-web-ui/) | unchanged by the layout change |

## Deleted

| Old path | New path | Recover from |
|---|---|---|
| `docs/datalink-mp rc4 tests.md` | deleted | `pre-layout` |
| `crates/smac-helper/prototype/ui-flow-prototype.html` | deleted | `pre-layout` (also commit `6399164`, which ADR-0001 cites) |
| `docs/datalink-mp-rc6-test-checklist.html` | deleted, never tracked | not recoverable |
| `docs/capture_test_launch.txt` | deleted, never tracked; the launch command is in [`docs/contributors/traffic-capture.md`](../contributors/traffic-capture.md) | not recoverable |

## Branches kept as tags

| Branch | Tag |
|---|---|
| `research/windows-toolchain` | `archive/research-windows-toolchain` |
| `research/macos-artifact` | `archive/research-macos-artifact` |
| `research/av-risk` | `archive/research-av-risk` |
| `prototype/turn-sync-activity` | `archive/prototype-turn-sync-activity` |

## Moved live files

Not archived, listed so old links can be followed. `git log --follow <new path>`
shows each file's history across the move.

| Old path | New path |
|---|---|
| `datalink-mp-README.txt` | [`packaging/README.txt`](../../packaging/README.txt) (still ships in the archives as `datalink-mp-README.txt`) |
| `docs/ARCHITECTURE.md` | [`docs/contributors/architecture.md`](../contributors/architecture.md) |
| `docs/building.md` | [`docs/contributors/building.md`](../contributors/building.md) |
| `docs/traffic-capture.md` | [`docs/contributors/traffic-capture.md`](../contributors/traffic-capture.md) |
| `docs/releasing.md` | [`docs/maintainers/releasing.md`](../maintainers/releasing.md) |
| `docs/datalink-mp-capture-test-checklist.html` | [`docs/maintainers/capture-test-checklist.html`](../maintainers/capture-test-checklist.html) |
| `docs/faction-colors.md` | [`docs/players/faction-colors.md`](../players/faction-colors.md) |
| `docs/wine-compatibility.md` | [`docs/players/wine-compatibility.md`](../players/wine-compatibility.md) |
| `docs/images/` | `docs/players/images/` |
