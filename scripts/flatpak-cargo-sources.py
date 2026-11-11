#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Writes cargo-sources.json for flatpak-builder from Cargo.lock: every crates.io package as
an archive checked by the sha256 in the lock file, unpacked into cargo/vendor, plus the Cargo
config that builds from there without a network. Same layout as flatpak-builder-tools'
flatpak-cargo-generator, for a lock file with crates.io packages only (it refuses others).

    scripts/flatpak-cargo-sources.py Cargo.lock > apps/linux/packaging/flatpak/cargo-sources.json
"""
import json
import sys
import tomllib

VENDOR = "cargo/vendor"
REGISTRY = "registry+https://github.com/rust-lang/crates.io-index"


def sources(lock: dict) -> list:
    out = []
    for pkg in lock["package"]:
        source = pkg.get("source")
        if source is None:
            continue  # one of this workspace's own crates
        if source != REGISTRY:
            sys.exit(f"unsupported source for {pkg['name']}: {source}")
        name, version, sha = pkg["name"], pkg["version"], pkg["checksum"]
        dest = f"{VENDOR}/{name}-{version}"
        out.append({
            "type": "archive",
            "archive-type": "tar-gzip",
            "url": f"https://static.crates.io/crates/{name}/{name}-{version}.crate",
            "sha256": sha,
            "dest": dest,
        })
        out.append({
            "type": "inline",
            "contents": json.dumps({"package": sha, "files": {}}),
            "dest": dest,
            "dest-filename": ".cargo-checksum.json",
        })
    out.append({
        "type": "inline",
        "contents": (
            '[source.vendored-sources]\ndirectory = "cargo/vendor"\n\n'
            '[source.crates-io]\nreplace-with = "vendored-sources"\n'
        ),
        "dest": "cargo",
        "dest-filename": "config.toml",
    })
    return out


if __name__ == "__main__":
    with open(sys.argv[1] if len(sys.argv) > 1 else "Cargo.lock", "rb") as f:
        print(json.dumps(sources(tomllib.load(f)), indent=1))
