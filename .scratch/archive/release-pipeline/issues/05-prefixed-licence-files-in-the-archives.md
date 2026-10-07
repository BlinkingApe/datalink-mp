# 05: Prefixed licence files in the archives

**What to build:** Found during the 0.1.0 gate. The helper-web-ui spec's archive layout says every file but the two binaries carries a `datalink-mp-` prefix, so extracting into the Game folder can't overwrite the game's own readme or licence files, and `datalink-mp-README.txt` refers to `datalink-mp-LICENSE-MIT.txt` and `datalink-mp-LICENSE-APACHE.txt`. `release.yml` packed them as `LICENSE-MIT` and `LICENSE-APACHE` instead, in all three archives of `v0.1.0-rc.2`.

`release.yml` now copies the licences in under the prefixed names, along with the new `datalink-mp-LICENSE-FONTS.txt` (helper-web-ui ticket 18).

**Blocked by:** None

**Status:** resolved

- [x] The packing step, run locally with stand-in binaries, gives a `.zip` and a `.tar.gz` holding the Helper, `dplayx.dll`, `datalink-mp-README.txt` and the three `datalink-mp-LICENSE-*.txt` files, with the Linux Helper still executable
- [x] The `-rc.3` archives (Windows, Linux, macOS) list those files

## Comments

**2026-10-01 (agent):** Checked on `v0.1.0-rc.3` (https://github.com/BlinkingApe/datalink-mp/actions/runs/36905778403). All three archives hold the Helper, `dplayx.dll`, `datalink-mp-README.txt`, `datalink-mp-LICENSE-MIT.txt`, `datalink-mp-LICENSE-APACHE.txt` and `datalink-mp-LICENSE-FONTS.txt`. The Linux Helper extracts executable.
