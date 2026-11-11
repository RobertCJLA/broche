#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Self-test for check-public.sh: runs it on throwaway repositories.
set -euo pipefail

here=$(cd "$(dirname "$0")" && pwd)
fails=0

# case <name> <expected exit> <file> <content> [words]
# ADDRS stands in for this machine's addresses; WORDS_BLANK=1 writes a blank words file.
ADDRS=""
case_() {
    local name=$1 want=$2 file=$3 content=$4 words=${5-}
    local dir
    dir=$(mktemp -d)
    git -C "$dir" init -q
    mkdir -p "$(dirname "$dir/$file")"
    printf '%b\n' "$content" > "$dir/$file"
    git -C "$dir" add -f -- "$file"
    local args=(--words "$dir/.no-words")
    if [ -n "${WORDS_BLANK-}" ]; then
        printf '\n  \n' > "$dir/.words"
        args=(--words "$dir/.words")
    elif [ -n "$words" ]; then
        printf '%s\n' "$words" > "$dir/.words"
        args=(--words "$dir/.words")
    fi
    local out got=0
    out=$(cd "$dir" && CHECK_PUBLIC_ADDRS="$ADDRS" "$here/check-public.sh" "${args[@]}" 2>&1) || got=$?
    if [ "$got" = "$want" ] && { [ "$want" = 0 ] || grep -qF -- "$file" <<<"$out"; }; then
        echo "ok   $name"
    else
        echo "FAIL $name: exit $got, wanted $want"; echo "$out"; fails=$((fails + 1))
    fi
    rm -rf "$dir"
}

key="-----BEGIN OPENSSH ""PRIVATE KEY-----"
token="ghp""_abcdefghijklmnopqrstuvwxyz0123456789"

case_ "clean file"              0 clean.txt  "hello"
case_ "private key"             1 k.txt      "$key"
case_ "token, name with spaces" 1 "a b.txt"  "$token"
case_ "keystore file"           1 x.jks      "binary"
case_ "agent notes"             1 AGENTS.md  "notes"
case_ "a listed address"        1 ip.txt     "at 192.168.7.9" "192.168.7.9"
case_ "example address"         0 ex.txt     "192.168.1.20"
case_ "private word, any case"  1 w.txt      "by SecretName" "secretname"
case_ "no words file"           0 w2.txt     "by SecretName"
case_ "word inside a binary"    1 shot.png   "\\x89PNG\\x00\\x01 SecretName \\x00" "secretname"
case_ "notes in a subfolder"    1 apps/linux/CLAUDE.md "notes"
case_ "codex notes in a crate"  1 crates/x/.codex/a.txt "notes"
case_ "keystore in capitals"    1 Release.JKS "binary"
ADDRS="192.168.1.5"
case_ "this machine's address"  1 a.txt      "host 192.168.1.5 here"
case_ "longer address is fine"  0 b.txt      "host 192.168.1.50 here"
ADDRS=""
WORDS_BLANK=1 case_ "blank words file" 0 c.txt "hello"

[ "$fails" = 0 ] || { echo "$fails failed"; exit 1; }
