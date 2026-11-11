#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Builds every release file into target/release-files/ on this machine:
#   Broche-<v>-x86_64.flatpak, Broche-<v>-x86_64.AppImage,
#   Broche-<v>-linux-x86_64.tar.gz, Broche-<v>-android.apk, SHA256SUMS.txt
#
#   scripts/release-local.sh <version>
#
# Needs podman (COMPAT=1 build), appimagetool, flatpak-builder with the
# Freedesktop runtime, the Android SDK, and the Rust targets
# aarch64-linux-android and armv7-linux-androideabi (checked first). The APK is signed with the release
# key: BROCHE_KEYSTORE (default ~/.config/broche-dev/release.jks) and
# BROCHE_KEYSTORE_PASSWORD, asked for when not set.
set -euo pipefail

ver=${1:?usage: $0 <version>}
root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"
cargo_ver=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
[ "$ver" = "$cargo_ver" ] || { echo "Cargo.toml says $cargo_ver, not $ver" >&2; exit 1; }

# Everything the builds below need, checked before the long ones start.
for tool in podman appimagetool flatpak-builder; do
    command -v "$tool" >/dev/null || { echo "missing: $tool" >&2; exit 1; }
done
rustup=$(command -v rustup || echo "$HOME/.cargo/bin/rustup")
for target in aarch64-linux-android armv7-linux-androideabi; do  # the APK's arm64-v8a and armeabi-v7a
    "$rustup" target list --installed | grep -qx "$target" ||
        { echo "missing Rust target $target: rustup target add $target" >&2; exit 1; }
done

export BROCHE_KEYSTORE=${BROCHE_KEYSTORE:-${XDG_CONFIG_HOME:-$HOME/.config}/broche-dev/release.jks}
[ -f "$BROCHE_KEYSTORE" ] || { echo "no release key at $BROCHE_KEYSTORE" >&2; exit 1; }
if [ -z "${BROCHE_KEYSTORE_PASSWORD:-}" ]; then
    read -rsp "Release key password: " BROCHE_KEYSTORE_PASSWORD; echo
fi
export BROCHE_KEYSTORE_PASSWORD

out=target/release-files
rm -rf "$out"; mkdir -p "$out"
id=io.github.robertcjla.Broche

echo "== tarball and AppImage (Ubuntu 22.04 container)"
COMPAT=1 scripts/package-linux.sh
cp "target/package/Broche-$ver-linux-x86_64-compat.tar.gz" "$out/Broche-$ver-linux-x86_64.tar.gz"
cp "target/package/Broche-$ver-linux-x86_64-compat.AppImage" "$out/Broche-$ver-x86_64.AppImage"

echo "== Flatpak"
rm -rf target/flatpak/state/rofiles/rofiles-*
flatpak-builder --user --force-clean --state-dir=target/flatpak/state \
    --repo=target/flatpak/repo target/flatpak/release-build \
    apps/linux/packaging/flatpak/$id.yml
flatpak build-bundle target/flatpak/repo "$out/Broche-$ver-x86_64.flatpak" $id \
    --runtime-repo=https://dl.flathub.org/repo/flathub.flatpakrepo

echo "== Android"
(
    cd apps/android
    export JAVA_HOME=${JAVA_HOME:-$HOME/Android/jdk} ANDROID_HOME=${ANDROID_HOME:-$HOME/Android/sdk}
    export PATH=$JAVA_HOME/bin:$HOME/.cargo/bin:$PATH
    ./gradlew --no-daemon assembleRelease -Pbroche.abis=arm64-v8a,armeabi-v7a
)
apk=apps/android/app/build/outputs/apk/release/app-release.apk
apksigner=$(ls -d "${ANDROID_HOME:-$HOME/Android/sdk}"/build-tools/*/apksigner | sort -V | tail -1)
"$apksigner" verify --print-certs "$apk" | grep -q 'CN=Broche' ||
    { echo "the APK is not signed with the release key" >&2; exit 1; }
cp "$apk" "$out/Broche-$ver-android.apk"

(cd "$out" && sha256sum Broche-* > SHA256SUMS.txt)
for f in "Broche-$ver-x86_64.flatpak" "Broche-$ver-x86_64.AppImage" \
         "Broche-$ver-linux-x86_64.tar.gz" "Broche-$ver-android.apk" SHA256SUMS.txt; do
    [ -s "$out/$f" ] || { echo "missing $out/$f" >&2; exit 1; }
done
echo "== done:"; cat "$out/SHA256SUMS.txt"
