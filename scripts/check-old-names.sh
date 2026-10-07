#!/bin/bash
# Find old names left behind by the repo layout change (repo-layout-build 04;
# see the Checks section of CONTRIBUTING.md).
#
# Usage:
#   scripts/check-old-names.sh [--all] [-h|--help] [<pathspec>...]
#
# Searches tracked files for the pre-layout crate names (ipc-protocol,
# iroh-transport, smac-fixes, mock-dp-client, in hyphen and underscore form),
# datalink-mp-README.txt and tools/. Prints each hit that the allowlist doesn't
# cover as path:line: text, then a summary. Exits 1 if any such hit remains.
#
#   --all        also print allowlisted hits, prefixed "allowed " (any position)
#   -h, --help   print this usage and exit
#   <pathspec>   search only these paths (git pathspecs), e.g. .scratch/foo/;
#                one that matches no tracked file is an error (exit 2)
#
# The allowlist is scripts/old-names-allowlist.txt (OLD_NAMES_ALLOWLIST
# overrides it); its header explains the format.
set -euo pipefail

ALL=false
SPECS=()
for arg in "$@"; do
  case "$arg" in
    --all) ALL=true ;;
    -h|--help) sed -n '2,/^set -euo/{/^set -euo/d;s/^# \{0,1\}//;p}' "$0"; exit 0 ;;
    *) SPECS+=("$arg") ;;
  esac
done
set -- ${SPECS[@]+"${SPECS[@]}"}

ROOT="$(git rev-parse --show-toplevel)"

# An old name must not be the tail of a longer word (devtools/, rust-tools/),
# so datalink-transport, my_tools/ and the like never match.
EDGE='(^|[^[:alnum:]_-])'
CRATES='(ipc[-_]protocol|iroh[-_]transport|smac[-_]fixes|mock[-_]dp[-_]client)'
PATTERN="$EDGE$CRATES([^[:alnum:]]|\$)|datalink-mp-README\.txt|${EDGE}tools/"

ALLOWLIST="${OLD_NAMES_ALLOWLIST:-$ROOT/scripts/old-names-allowlist.txt}"
GLOBS=() REGEXES=()
while IFS= read -r entry || [[ -n "$entry" ]]; do
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

# A pathspec that names no tracked file is a typo, not a clean result.
for spec in "$@"; do
  if ! git ls-files --error-unmatch -- "$spec" > /dev/null 2>&1; then
    echo "check-old-names: pathspec '$spec' matches no tracked file" >&2
    exit 2
  fi
done

# git grep exits 1 for no matches; anything above that is a real error. It
# also exits 0 or 1 after a file it couldn't read, so any stderr is one too.
HITS="$(mktemp)" ERRS="$(mktemp)"
trap 'rm -f "$HITS" "$ERRS"' EXIT
STATUS=0
git grep -I -n -z --full-name -E "$PATTERN" -- "$@" > "$HITS" 2> "$ERRS" || STATUS=$?
if (( STATUS > 1 )) || [[ -s "$ERRS" ]]; then
  cat "$ERRS" >&2
  echo "check-old-names: git grep failed (exit $STATUS)" >&2
  exit 2
fi

FOUND=0 ALLOWED=0
while IFS= read -r -d '' path && IFS= read -r -d '' line && IFS= read -r text; do
  if allowed "$path" "$text"; then
    ALLOWED=$((ALLOWED + 1))
    if $ALL; then echo "allowed $path:$line: $text"; fi
  else
    FOUND=$((FOUND + 1))
    echo "$path:$line: $text"
  fi
done < "$HITS"

echo "$FOUND hit(s) not on the allowlist, $ALLOWED allowlisted"
[[ $FOUND -eq 0 ]]
