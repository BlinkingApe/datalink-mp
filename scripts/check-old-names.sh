#!/bin/bash
# Find old names left behind by the repo layout change (.scratch/repo-layout/spec.md, "Proof").
#
# Usage:
#   scripts/check-old-names.sh [--all] [<pathspec>...]
#
# Searches tracked files for the pre-layout crate names (ipc-protocol,
# iroh-transport, smac-fixes, mock-dp-client, in hyphen and underscore form),
# datalink-mp-README.txt and tools/. Prints each hit that the allowlist doesn't
# cover as path:line: text, then a summary. Exits 1 if any such hit remains.
#
#   --all        also print allowlisted hits, prefixed "allowed "
#   <pathspec>   search only these paths (git pathspecs), e.g. .scratch/foo/
#
# The allowlist is scripts/old-names-allowlist.txt (OLD_NAMES_ALLOWLIST
# overrides it); its header explains the format.
set -euo pipefail

ALL=false
if [[ "${1:-}" == --all ]]; then ALL=true; shift; fi

ROOT="$(git rev-parse --show-toplevel)"

# An old name must not be the tail of a longer word (devtools/, rust-tools/),
# so datalink-transport, my_tools/ and the like never match.
EDGE='(^|[^[:alnum:]_-])'
CRATES='(ipc[-_]protocol|iroh[-_]transport|smac[-_]fixes|mock[-_]dp[-_]client)'
PATTERN="$EDGE$CRATES([^[:alnum:]]|\$)|datalink-mp-README\.txt|${EDGE}tools/"

ALLOWLIST="${OLD_NAMES_ALLOWLIST:-$ROOT/scripts/old-names-allowlist.txt}"
GLOBS=() REGEXES=()
while IFS= read -r entry; do
  entry="${entry%%[[:space:]]#*}"
  [[ "$entry" =~ ^[[:space:]]*(#|$) ]] && continue
  read -r glob regex <<< "$entry"
  GLOBS+=("$glob") REGEXES+=("$regex")
done < "$ALLOWLIST"

# matches <i> <path> <text>: entry i's glob matches the path and its regex (if any) the text.
matches() {
  local glob="${GLOBS[$1]#!}" regex="${REGEXES[$1]}"
  # shellcheck disable=SC2053 # the glob is a pattern on purpose
  [[ "$2" == $glob ]] && [[ -z "$regex" || "$3" =~ $regex ]]
}

# allowed <path> <text>: an allow entry matches and no ! entry does.
allowed() {
  local i ok=1
  for i in "${!GLOBS[@]}"; do
    matches "$i" "$1" "$2" || continue
    [[ "${GLOBS[i]}" == !* ]] && return 1
    ok=0
  done
  return $ok
}

FOUND=0 ALLOWED=0
while IFS= read -r -d '' path && IFS= read -r -d '' line && IFS= read -r text; do
  if allowed "$path" "$text"; then
    ALLOWED=$((ALLOWED + 1))
    if $ALL; then echo "allowed $path:$line: $text"; fi
  else
    FOUND=$((FOUND + 1))
    echo "$path:$line: $text"
  fi
done < <(git grep -I -n -z --full-name -E "$PATTERN" -- "$@" || true)

echo "$FOUND hit(s) not on the allowlist, $ALLOWED allowlisted"
[[ $FOUND -eq 0 ]]
