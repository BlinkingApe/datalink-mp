#!/bin/bash
# Check a release tag against the workspace version (.github/workflows/release.yml).
#
# Usage:
#   scripts/release-version.sh <tag>
#
# Accepts vX.Y.Z and vX.Y.Z-rc.N, where X.Y.Z is the workspace version, so an
# RC's binaries already report the version it becomes. Any other tag fails.
# On success prints, in the form $GITHUB_OUTPUT takes:
#   version=X.Y.Z
#   prerelease=true|false
set -euo pipefail

TAG="${1:?usage: release-version.sh <tag>}"

WORKSPACE_VERSION="$(cargo metadata --no-deps --format-version 1 \
  | jq -r '.packages[] | select(.name == "datalink-mp") | .version')"

if [[ "$TAG" =~ ^v(.+)-rc\.[0-9]+$ ]]; then
  VERSION="${BASH_REMATCH[1]}"
  PRERELEASE=true
elif [[ "$TAG" =~ ^v(.+)$ ]]; then
  VERSION="${BASH_REMATCH[1]}"
  PRERELEASE=false
else
  echo "error: tag $TAG doesn't start with v" >&2
  exit 1
fi

if [[ "$VERSION" != "$WORKSPACE_VERSION" ]]; then
  echo "error: tag $TAG doesn't match the workspace version $WORKSPACE_VERSION" \
    "(expected v$WORKSPACE_VERSION or v$WORKSPACE_VERSION-rc.N)" >&2
  exit 1
fi

echo "version=$VERSION"
echo "prerelease=$PRERELEASE"
