#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Refuses to publish private data. Checks the files staged for the next commit,
# or every tracked file with --all (every file below "." outside a repository).
#
#   scripts/check-public.sh [--all] [--words FILE]
#
# FILE lists private words (names, e-mail, home folder), one per line, matched
# case-insensitively. It lives outside the repository so the words themselves
# are never published (default: ~/.config/broche-dev/private-words). Without
# it only the generic rules run. Install as a hook with:
#   ln -s ../../scripts/check-public.sh .git/hooks/pre-commit
set -euo pipefail

all=0
words="${XDG_CONFIG_HOME:-$HOME/.config}/broche-dev/private-words"
while [ $# -gt 0 ]; do
    case $1 in
        --all) all=1 ;;
        --words) words=$2; shift ;;
        *) echo "usage: $0 [--all] [--words FILE]" >&2; exit 2 ;;
    esac
    shift
done

in_repo=0
git rev-parse --is-inside-work-tree >/dev/null 2>&1 && in_repo=1

list() {
    if [ $all = 1 ] && [ $in_repo = 1 ]; then git ls-files -z
    elif [ $all = 1 ]; then find . -type f -not -path './.git/*' -printf '%P\0'
    else git diff --cached --name-only -z --diff-filter=ACMR
    fi
}

# The staged version for a commit, the file itself otherwise.
content() {
    if [ $all = 0 ]; then git show ":$1"; else cat -- "$1"; fi
}

secret='-----BEGIN [A-Z ]*PRIVATE KEY-----|\b(ghp|gho|ghs|ghu)_[A-Za-z0-9]{36}|github_pat_[A-Za-z0-9_]{20,}|glpat-[A-Za-z0-9_-]{20}|\bAKIA[0-9A-Z]{16}\b'
found=0

# Private words (matched anywhere) and this machine's own addresses (matched as
# whole addresses: example addresses in code and tests are fine, the real ones
# are not). 172.17.0.1 is the container bridge every machine has.
# CHECK_PUBLIC_ADDRS replaces `hostname -I` (the self-test sets it).
patterns=$(mktemp)
addrs=$(mktemp)
trap 'rm -f "$patterns" "$addrs"' EXIT
if [ -f "$words" ]; then grep -v '^[[:space:]]*$' "$words" > "$patterns" || true; fi
printf '%s\n' "${CHECK_PUBLIC_ADDRS-$(hostname -I 2>/dev/null)}" | tr ' ' '\n' |
    grep -vxE '|172\.17\.0\.1' > "$addrs" || true

hit() { echo "$1: $2" >&2; found=1; }

while IFS= read -r -d '' f; do
    # Any folder depth, any case.
    lower=/$(printf '%s' "$f" | tr '[:upper:]' '[:lower:]')
    case $lower in
        *.jks|*.keystore|*.p12|*.pfx|*.db|*.sqlite|*/identity.key|*/settings.conf)
            hit "$f" "secret or private data file"; continue ;;
        */agents.md|*/claude.md|*/.codex/*|*/docs/superpowers/*|*/docs/android_handoff.md|*/docs/branding_plan.md)
            hit "$f" "maintainer notes are kept off the repository"; continue ;;
    esac
    # These two files hold the patterns themselves.
    case $f in scripts/check-public.sh|scripts/check-public-test.sh) continue ;; esac
    # -a: binary files (screenshots) are searched too.
    text=$(content "$f" | grep -naE -e "$secret" || true)
    [ -n "$text" ] && while IFS= read -r l; do hit "$f:${l%%:*}" "private key or token"; done <<<"$text"
    if [ -s "$patterns" ]; then
        text=$(content "$f" | grep -naiF -f "$patterns" || true)
        [ -n "$text" ] && while IFS= read -r l; do hit "$f:${l%%:*}" "private word"; done <<<"$text"
    fi
    if [ -s "$addrs" ]; then
        text=$(content "$f" | grep -nawF -f "$addrs" || true)
        [ -n "$text" ] && while IFS= read -r l; do hit "$f:${l%%:*}" "this machine's address"; done <<<"$text"
    fi
done < <(list)

if [ $found = 1 ]; then
    echo "check-public: private data found, nothing may be published like this." >&2
    exit 1
fi
