#!/bin/sh
# SPDX-License-Identifier: GPL-3.0-or-later
# Sets the project version everywhere it is written down and moves the
# "## Unreleased" notes of CHANGELOG.md under it (see docs/VERSIONING.md).
# With the version it already has (0.2.0, never published), it only folds the
# notes into that version's section and dates it today.
#   scripts/bump-version.sh 0.2.0
set -eu
NEW="${1:-}"
case "$NEW" in
  [0-9]*.[0-9]*.[0-9]*) ;;
  *) echo "uso / usage: $0 MAJOR.MINOR.PATCH" >&2; exit 2 ;;
esac
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
OLD="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)"
DATE="$(date +%F)"
META=apps/linux/data/io.github.robertcjla.Broche.metainfo.xml
grep -q '^## Unreleased$' CHANGELOG.md || { echo "CHANGELOG.md has no '## Unreleased' section" >&2; exit 1; }

if [ "$OLD" = "$NEW" ]; then
  # The version was set ahead of time but never published (0.2.0): fold the
  # Unreleased notes into its section and date it today. A tagged version is
  # published and never changes.
  if git rev-parse -q --verify "refs/tags/v$NEW" >/dev/null; then
    echo "v$NEW is already tagged: pick a new version" >&2; exit 1
  fi
  grep -q "^## $NEW (" CHANGELOG.md || { echo "CHANGELOG.md has no '## $NEW (…)' section" >&2; exit 1; }
  # Its introduction (summary bullets included) stays first; the newer notes go
  # after it, before its older notes, which start at the first "### " heading.
  awk -v new="$NEW" -v date="$DATE" '
    function flush() { if (!done) { sub(/\n+$/, "\n", notes); printf "%s", notes; done = 1 } }
    state == 0 && $0 == "## Unreleased" { print; print ""; state = 1; next }
    state == 1 && index($0, "## " new " (") == 1 { print "## " new " (" date ")"; state = 2; next }
    state == 1 { if (notes != "" || $0 != "") notes = notes $0 "\n"; next }
    state == 2 && (/^### / || /^## /) { flush(); print ""; state = 3 }
    { print }
    END { if (state == 2) flush() }
  ' CHANGELOG.md > CHANGELOG.md.tmp && mv CHANGELOG.md.tmp CHANGELOG.md
  sed -i "s|<release version=\"$NEW\" date=\"[0-9-]*\"/>|<release version=\"$NEW\" date=\"$DATE\"/>|" "$META"
  echo "$NEW dated $DATE. Revisa / review: git diff"
  exit 0
fi

# Workspace version and the internal crates' `version = "…"` references.
sed -i "s/^version = \"$OLD\"/version = \"$NEW\"/" Cargo.toml
sed -i "/^broche-[a-z]* = { path = /s/version = \"$OLD\"/version = \"$NEW\"/" Cargo.toml

# AppStream metadata of the Linux app: newest release first.
sed -i "s|<releases>|<releases><release version=\"$NEW\" date=\"$DATE\"/>|" "$META"

# CHANGELOG: the Unreleased notes become this version's; a new empty
# Unreleased section goes on top.
sed -i "s/^## Unreleased$/## Unreleased\n\n## $NEW ($DATE)/" CHANGELOG.md

. "$HOME/.cargo/env" 2>/dev/null || true
cargo update --workspace --offline >/dev/null 2>&1 || cargo update --workspace >/dev/null

echo "$OLD → $NEW. Revisa / review: git diff"
