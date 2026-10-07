#!/bin/bash
# Tests for the layout proof scripts (check-old-names.sh, check-links.py).
#
# Usage:
#   scripts/test-proof-scripts.sh
#
# Each test builds a throwaway git repo, runs a script inside it and checks the
# exit code and output. Prints one line per test; exits non-zero if any fails.
set -uo pipefail

SCRIPTS="$(cd "$(dirname "$0")" && pwd)"
GREP="$SCRIPTS/check-old-names.sh"
LINKS="$SCRIPTS/check-links.py"
FAILED=0
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

# fixture <path=content>...: a fresh git repo with these files staged, and an
# empty allowlist for both scripts. Prints its directory.
fixture() {
  local dir pair path
  dir="$(mktemp -d "$TMP/repo.XXXXXX")"
  git -C "$dir" init -q
  for pair in "$@"; do
    path="${pair%%=*}"
    mkdir -p "$dir/$(dirname "$path")"
    printf '%s\n' "${pair#*=}" > "$dir/$path"
  done
  git -C "$dir" add -A
  echo "$dir"
}

# allow <dir> <line>...: the allowlist both scripts use in this fixture.
allow() {
  local dir="$1"; shift
  printf '%s\n' "$@" > "$dir/.allow"
}

# run <dir> <command>...: sets OUT and CODE.
run() {
  local dir="$1"; shift
  [[ -e "$dir/.allow" ]] || : > "$dir/.allow"
  OUT="$(cd "$dir" && OLD_NAMES_ALLOWLIST="$dir/.allow" LINKS_ALLOWLIST="$dir/.allow" "$@" 2>&1)"
  CODE=$?
}

has() { grep -qF -- "$1" <<< "$OUT" && echo yes || echo no; }
lacks() { grep -qF -- "$1" <<< "$OUT" && echo no || echo yes; }
code() { [[ "$CODE" == "$1" ]] && echo yes || echo no; }

# check <name> <yes|no>...: passes when every condition is yes.
check() {
  local name="$1"; shift
  if [[ " $* " != *" no "* ]]; then
    echo "ok   $name"
  else
    echo "FAIL $name"
    echo "     exit $CODE, output:"
    sed 's/^/     | /' <<< "$OUT"
    FAILED=1
  fi
}

# --- check-old-names.sh ---

D="$(fixture 'src/lib.rs=use iroh_transport::Ticket;' 'README.md=nothing old here')"
run "$D" "$GREP"
check "grep fails on an old name outside the allowlist" \
  "$(code 1)" "$(has 'src/lib.rs:1:')"

D="$(fixture 'README.md=nothing old here' 'src/lib.rs=use datalink_transport::Ticket;')"
run "$D" "$GREP"
check "grep passes when only new names remain" "$(code 0)"

D="$(fixture "old.md=ipc-protocol
ipc_protocol
iroh-transport
iroh_transport
smac-fixes
smac_fixes
mock-dp-client
mock_dp_client
cp datalink-mp-README.txt dist/
see tools/0001-wtp.html
cargo run --manifest-path ./tools/mock/Cargo.toml
RUST_LOG=iroh_transport=debug")"
run "$D" "$GREP"
SEEN=()
for n in {1..12}; do SEEN+=("$(has "old.md:$n:")"); done
check "grep catches every old name, hyphen and underscore, README and tools/" \
  "$(code 1)" "${SEEN[@]}"

D="$(fixture "new.md=datalink-transport datalink_transport datalink-ipc datalink-fixes
datalink-mock-client packaging/README.txt devtools/ rust-tools/x my_tools/y
the iroh transport layer; iroh::transport")"
run "$D" "$GREP"
check "grep ignores new names and words that merely contain an old one" "$(code 0)"

D="$(fixture 'docs/adr/0001-x.md=we use ipc-protocol' '.scratch/archive/a/b.md=smac_fixes' \
  'docs/live.md=smac-fixes is live')"
allow "$D" '# comment' '' 'docs/adr/*' '.scratch/archive/**   # archived history'
run "$D" "$GREP"
check "grep skips files matched by an allowlist glob, and only those" \
  "$(code 1)" "$(has 'docs/live.md:1:')" "$(lacks 'docs/adr/')" "$(lacks '.scratch/archive/')"

D="$(fixture 'ci.yml=cp packaging/README.txt "$d/datalink-mp-README.txt"
cp datalink-mp-README.txt "$d/"')"
allow "$D" 'ci.yml   packaging/README\.txt.*datalink-mp-README\.txt   # shipped name'
run "$D" "$GREP"
check "grep allows a hit only when its line matches the entry's regex" \
  "$(code 1)" "$(lacks 'ci.yml:1:')" "$(has 'ci.yml:2:')"

D="$(fixture 'docs/adr/0001.md=we chose iroh_transport::Ticket
the mesh test in `crates/iroh-transport`')"
allow "$D" 'docs/adr/*' '!docs/adr/*  crates/iroh-transport'
run "$D" "$GREP"
check "grep reports a hit matching a ! entry even when another entry allows it" \
  "$(code 1)" "$(lacks '0001.md:1:')" "$(has '0001.md:2:')"

D="$(fixture 'docs/adr/0001.md=ipc-protocol' 'docs/live.md=smac-fixes')"
allow "$D" 'docs/adr/*'
run "$D" "$GREP" --all
check "grep --all also lists allowlisted hits" \
  "$(has 'allowed docs/adr/0001.md:1:')" "$(has 'docs/live.md:1:')"
run "$D" "$GREP" docs/adr/
check "grep with a pathspec searches only those paths" "$(code 0)"

# --- check-links.py ---

D="$(fixture 'README.md=See [building](docs/building.md) and [gone](docs/gone.md).' \
  'docs/building.md=Back to [the readme](../README.md#top).')"
run "$D" "$LINKS"
check "links: a relative markdown link to a missing file fails, a good one passes" \
  "$(code 1)" "$(has 'README.md:1: docs/gone.md')" "$(lacks 'building.md')"

D="$(fixture 'docs/a b.md=x' 'docs/img/p.png=x' 'docs/index.md=[web](https://example.com/x.md) [mail](mailto:a@b.c) [here](#top)
[dir](img/) [spaced](a%20b.md) [angle](<a b.md>) ![pic](img/p.png "title")
<img src="img/p.png"> <img src="img/missing.png">
[ref]: ../nowhere.md
`[not a link](gone1.md)` is code
```
[also code](gone2.md)
```' 'crates/x/README.md=[out of scope](gone3.md)')"
run "$D" "$LINKS"
check "links: resolves dirs, %20, <>, images and src=, skips URLs, anchors and code" \
  "$(has 'docs/index.md:3: img/missing.png')" "$(has 'docs/index.md:4: ../nowhere.md')" \
  "$(lacks 'example.com')" "$(lacks 'mailto')" "$(lacks ':2:')" "$(lacks 'gone')"

D="$(fixture 'docs/x.md=x' 'crates/a/src/lib.rs=x' '.scratch/e/analysis/x.py=x' '.scratch/e/map.md=x' \
  '.scratch/e/issues/01.md=Gone: `docs/gone.md`, `crates/old/`, `crates/a/src/nope.rs`.
Fine: `crates/a/src/lib.rs:12`, `crates/a/src/lib.rs:39-44`, `src/lib.rs`, `crates/a/`,
`../analysis/x.py`, `.scratch/e/map.md`, `lib.rs`, `map.md`, `AGENTS.md`, `endpoint.rs:12`.
Not repo paths: `tokio-1.53.0/src/rt.rs`, `~/x/y.md`, `/usr/bin/open`, `.scratch/<effort>/`,
`crates/*`, `Trojan:Script/W.B`, `release.yml@refs/tags/v1`, `LOG=/path`, `a b/c.md`, `TCP/UDP`, `n/a`
```
cd `crates/gone-in-fence`
```')"
run "$D" "$LINKS"
check "links: backticked repo paths resolve against the file, the root or a tracked suffix" \
  "$(code 1)" "$(has '01.md:1: `docs/gone.md`')" "$(has '01.md:1: `crates/old/`')" \
  "$(has '01.md:1: `crates/a/src/nope.rs`')" "$(lacks ':2:')" "$(lacks ':3:')" \
  "$(lacks ':4:')" "$(lacks ':5:')" "$(lacks 'fence')"

D="$(fixture 'docs/research/x.md=x' '.scratch/archive/a.md=x' 'docs/b.md=Built on `research/foo`, see `prototype/ui-flow/serve.py`,
`origin/main`, `capture/traffic-capture`, `research/{a,b}`.
[tree](https://github.com/o/r/tree/research/foo/docs/x.md) [blob](https://github.com/o/r/blob/main/README.md)
Tags are fine: `archive/prototype-turn-sync`, [tag](https://github.com/o/r/tree/v0.1.0/x),
[commit](https://github.com/o/r/blob/0123abc/x.md), other projects: [x](https://github.com/x/y/blob/main/a.md)')"
git -C "$D" remote add origin https://github.com/o/r.git
run "$D" "$LINKS"
check "links: branch references are listed separately and don't fail the check" \
  "$(code 0)" "$(has 'branch docs/b.md:1: `research/foo`')" \
  "$(has 'branch docs/b.md:1: `prototype/ui-flow/serve.py`')" "$(has 'branch docs/b.md:2: `origin/main`')" \
  "$(has 'branch docs/b.md:2: `capture/traffic-capture`')" "$(has 'branch docs/b.md:2: `research/{a,b}`')" \
  "$(has 'branch docs/b.md:3: https://github.com/o/r/tree/research/foo/docs/x.md')" \
  "$(has 'branch docs/b.md:3: https://github.com/o/r/blob/main/README.md')" \
  "$(lacks ':4:')" "$(lacks ':5:')"

D="$(fixture 'docs/x.md=x' '.scratch/archive/old.md=[gone](gone.md) `docs/gone.md` `research/old`' \
  '.scratch/live/a.md=`docs/gone.md` `.scratch/live/captures/`' '.scratch/live/b.md=[gone](gone.md)')"
allow "$D" '.scratch/archive/*' '.scratch/*   captures/$   # gitignored'
run "$D" "$LINKS"
check "links: the allowlist excuses whole files or matching targets only" \
  "$(code 1)" "$(lacks 'archive')" "$(lacks 'captures')" "$(has '.scratch/live/a.md:1: `docs/gone.md`')" \
  "$(has '2 broken link(s) not on the allowlist, 4 allowlisted, 0 branch reference(s)')"
run "$D" "$LINKS" --all
check "links: --all also lists allowlisted broken links" \
  "$(has 'allowed .scratch/archive/old.md:1: gone.md')"
run "$D" "$LINKS" .scratch/live/a.md .scratch/archive/
check "links: path arguments limit the files checked" \
  "$(code 1)" "$(lacks 'b.md')" "$(has '1 broken link(s)')"

exit "$FAILED"
