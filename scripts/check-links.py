#!/usr/bin/env python3
"""Check that relative links in the repo's markdown resolve (repo-layout-build
04; see the Checks section of CONTRIBUTING.md).

Usage:
  scripts/check-links.py [--all] [-h|--help] [<path>...]

Reads the tracked *.md files at the root (README, CONTRIBUTING, CLAUDE.md, ...),
under docs/ (ADRs included) and under .scratch/, and checks against the index
(stage a move with git mv or git add before checking it):

  - every relative markdown link, reference definition and src=/href= target
    (URLs and pure #anchors are skipped; anchors are not checked)
  - every backticked repo path, such as `docs/building.md` or
    `crates/dplayx/src/lib.rs:12`. Heuristic: a token counts only if it has a
    / and starts with ./, ../, a tracked top-level name (anchored: resolved
    against the file's folder, then the root) or a tracked folder name (loose:
    also accepted as the tail of any tracked path, so `src/http.rs` passes).
    Bare file names, absolute paths, placeholders (<x>, *, {}) and code inside
    fenced blocks are not checked.

Prints each broken link not on the allowlist as path:line: target, then each
branch reference not on the allowlist ("branch " prefix: a GitHub tree/blob URL
into a branch of this repo or one of its remotes, or a backticked research/,
prototype/, capture/, origin/, upstream/ or worktree-agent- name) for review by
hand, then a summary. Exits 1 if any broken link remains; branch references
alone don't fail.

  --all       also print allowlisted broken links and branch references,
              prefixed "allowed " (any position)
  -h, --help  print this usage and exit
  <path>      check only these files or folders; one that names no tracked
              file is an error (exit 2)

The allowlist is scripts/links-allowlist.txt (LINKS_ALLOWLIST overrides it), in
the format of scripts/old-names-allowlist.txt; an entry's regex (Python syntax)
is searched for in the link target.
"""
import fnmatch
import os
import re
import subprocess
import sys
from urllib.parse import unquote

ROOT = subprocess.run(["git", "rev-parse", "--show-toplevel"],
                      capture_output=True, text=True, check=True).stdout.strip()

# Markdown files in scope: root *.md, docs/ (ADRs included) and .scratch/.
SCOPE = re.compile(r"^([^/]+\.md|docs/.+\.md|\.scratch/.+\.md)$")

# A link title may be "double-quoted", 'single-quoted' or (parenthesised).
LINK = re.compile(r"!?\[[^\]]*\]\(\s*(<[^>]*>|[^)\s]+)"
                  r"(?:\s+(?:\"[^\"]*\"|'[^']*'|\([^)]*\)))?\s*\)")
REF_DEF = re.compile(r"^\s{0,3}\[[^\]]+\]:\s*(\S+)")
HTML_ATTR = re.compile(r"\b(?:src|href)=\"([^\"]+)\"")
CODE_SPAN = re.compile(r"(`+)(.+?)\1")
# CommonMark fences: an opening run of 3+ backticks (info string without
# backticks) or tildes; it closes only on a run of the same character at least
# as long, with nothing but spaces after it.
FENCE_OPEN = re.compile(r"^ {0,3}(`{3,}(?=[^`]*$)|~{3,})")
FENCE_CLOSE = re.compile(r"^ {0,3}(`{3,}|~{3,})[ \t]*$")
SCHEME = re.compile(r"^[A-Za-z][A-Za-z0-9+.-]*:")

# Branch references: a GitHub URL into this repo (any remote) at a ref that
# isn't a version tag, archive tag or commit hash, or a backticked
# branch-shaped name.
GITHUB_REF = re.compile(r"^https?://github\.com/([^/]+/[^/]+)/(?:tree|blob|commits|compare)/(.+)$")
GITHUB_REMOTE = re.compile(r"github\.com[:/]([^/\s]+/[^/\s]+?)(?:\.git)?\s")
NOT_BRANCH = re.compile(r"^(v\d|[0-9a-f]{7,40}(/|$)|archive/)")
BRANCH_NAME = re.compile(r"^((origin|upstream)/\S+|(research|prototype|capture)/\S+|worktree-agent-\S+)$")

# A backticked token is checked only if it looks like a repo path: path
# characters only, not absolute, with a / and a first segment that names a
# tracked top-level entry or directory. A trailing line reference (:12,
# :39-44, #L5) is dropped. Bare file names (lib.rs) are not checked.
PATHY = re.compile(r"^[A-Za-z0-9._/-]+$")
LINE_REF = re.compile(r"(:\d+([-,/]\d+)*|#L\d+(-L?\d+)?)$")
TAG = re.compile(r"^archive/(prototype|research)-")


def read_allowlist(path):
    """(deny, glob, regex) entries; the format is the same as old-names-allowlist.txt."""
    entries = []
    with open(path, encoding="utf-8") as f:
        for line in f:
            line = re.sub(r"\s#.*", "", line).strip()
            if not line or line.startswith("#"):
                continue
            glob, regex = (line.split(None, 1) + [""])[:2]
            entries.append((glob.startswith("!"), glob.lstrip("!"),
                            re.compile(regex) if regex else None))
    return entries


def allowed(entries, path, target):
    """An allow entry matches the file (and target) and no deny entry does."""
    ok = False
    for deny, glob, regex in entries:
        if fnmatch.fnmatchcase(path, glob) and (regex is None or regex.search(target)):
            if deny:
                return False
            ok = True
    return ok


def tracked():
    out = subprocess.run(["git", "ls-files", "-z"], cwd=ROOT,
                         capture_output=True, text=True, check=True).stdout
    return [p for p in out.split("\0") if p]


class Tree:
    """What exists in the index, for resolving paths."""

    def __init__(self, files):
        self.paths = set(files)
        for f in files:
            d = os.path.dirname(f)
            while d:
                self.paths.add(d)
                d = os.path.dirname(d)
        self.top = {p.split("/")[0] for p in self.paths}
        self.dir_names = {os.path.basename(p) for p in self.paths - set(files)}
        # Every path-component-aligned tail of every path: src/lib.rs, lib.rs, ...
        self.tails = set()
        for p in self.paths:
            parts = p.split("/")
            self.tails.update("/".join(parts[i:]) for i in range(len(parts)))

    def has(self, path):
        return os.path.normpath(path) in self.paths

    def link_resolves(self, local, here):
        base = "" if local.startswith("/") else here
        return self.has(os.path.join(base, local.lstrip("/")))

    def code_path_resolves(self, token, here):
        """True/False for a backticked repo path, None if it isn't one."""
        token = LINE_REF.sub("", token)
        if not PATHY.match(token) or token.startswith("/") or TAG.match(token):
            return None
        bare = token.rstrip("/")
        if "/" not in bare:
            return None
        first = bare.split("/")[0]
        if first in (".", "..") or first in self.top:
            anchored = True
        elif first in self.dir_names:
            anchored = False
        else:
            return None
        if self.has(os.path.join(here, bare)) or self.has(bare):
            return True
        return not anchored and os.path.normpath(bare) in self.tails


def link_targets(line):
    """Link targets on a line whose code spans are already removed."""
    targets = [t[1:-1] if t.startswith("<") else t for t in LINK.findall(line)]
    targets += HTML_ATTR.findall(line)
    m = REF_DEF.match(line)
    if m:
        targets.append(m.group(1))
    return targets


def local_part(target):
    """The repo path a link names, or None for URLs and pure anchors."""
    if SCHEME.match(target):
        return None
    target = unquote(target.split("#")[0].split("?")[0])
    return target or None


def github_repos():
    """owner/repo of every GitHub remote, lowercased."""
    out = subprocess.run(["git", "remote", "-v"], cwd=ROOT,
                         capture_output=True, text=True, check=True).stdout
    return {m.lower() for m in GITHUB_REMOTE.findall(out)}


REPOS = github_repos()


def points_at_branch(target):
    m = GITHUB_REF.match(target)
    return bool(m) and m.group(1).lower() in REPOS and not NOT_BRANCH.match(m.group(2))


def check_line(line, here, tree):
    """Yields (kind, target, shown) for each broken link or branch reference on a line."""
    for _, token in CODE_SPAN.findall(line):
        token = token.strip()
        if BRANCH_NAME.match(token):
            yield "branch", token, f"`{token}`"
        elif tree.code_path_resolves(token, here) is False:
            yield "broken", token, f"`{token}`"
    for target in link_targets(CODE_SPAN.sub("", line)):
        if points_at_branch(target):
            yield "branch", target, target
            continue
        local = local_part(target)
        if local is not None and not tree.link_resolves(local, here):
            yield "broken", target, target


def under(path, prefix):
    return path == prefix or path.startswith(prefix.rstrip("/") + "/")


def main(argv):
    if "-h" in argv or "--help" in argv:
        print(__doc__.strip())
        return 0
    show_allowed = "--all" in argv
    args = [a for a in argv if a != "--all"]
    only = [os.path.relpath(os.path.abspath(a), ROOT) for a in args]
    allowlist = read_allowlist(os.environ.get(
        "LINKS_ALLOWLIST", os.path.join(ROOT, "scripts", "links-allowlist.txt")))
    files = tracked()
    for arg, o in zip(args, only):
        if not any(under(p, o) for p in files):
            print(f"check-links: {arg} names no tracked file", file=sys.stderr)
            return 2
    tree = Tree(files)
    broken, excused, branches = [], [], []
    for path in files:
        if not SCOPE.match(path):
            continue
        if only and not any(under(path, o) for o in only):
            continue
        here = os.path.dirname(path)
        fence = None  # the open fence's run, such as ``` or ~~~~
        with open(os.path.join(ROOT, path), encoding="utf-8") as f:
            for n, line in enumerate(f, 1):
                if fence is not None:
                    m = FENCE_CLOSE.match(line)
                    if m and m.group(1)[0] == fence[0] and len(m.group(1)) >= len(fence):
                        fence = None
                    continue
                m = FENCE_OPEN.match(line)
                if m:
                    fence = m.group(1)
                    continue
                for kind, target, shown in check_line(line, here, tree):
                    found = f"{path}:{n}: {shown}"
                    if allowed(allowlist, path, target):
                        excused.append(found)
                    elif kind == "branch":
                        branches.append(found)
                    else:
                        broken.append(found)
    for b in broken:
        print(b)
    if show_allowed:
        for b in excused:
            print(f"allowed {b}")
    for b in branches:
        print(f"branch {b}")
    print(f"{len(broken)} broken link(s) not on the allowlist, {len(excused)} allowlisted, "
          f"{len(branches)} branch reference(s) to review")
    return 1 if broken else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
