#!/bin/sh
# Builds the Linux release: a tarball (with install.sh) and, when
# appimagetool is available, an AppImage.  Output goes to target/package/.
#   scripts/package-linux.sh            tarball (+ AppImage if possible)
#   APPIMAGE=0 scripts/package-linux.sh tarball only
#   COMPAT=1 scripts/package-linux.sh   build inside Ubuntu 22.04 (podman) so the binaries
#                                       run on older distributions too (glibc 2.35+)
set -eu
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
. "$HOME/.cargo/env" 2>/dev/null || true
VER="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)"
ARCH="$(uname -m)"
ID=io.github.robertcjla.Broche
OUT="$ROOT/target/package"
NAME="Broche-$VER-linux-$ARCH"
STAGE="$OUT/$NAME"
rm -rf "$STAGE"; mkdir -p "$STAGE/bin" "$OUT"

if [ "${COMPAT:-0}" = 1 ]; then
  # The build needs only a C compiler (ring, bundled SQLite). Cargo and rustup live in named
  # volumes, so later builds don't download them again.
  BUILT=target/compat/release
  podman run --rm -v "$ROOT":/src:Z -v broche-cargo:/root/.cargo -v broche-rustup:/root/.rustup \
    -w /src docker.io/library/ubuntu:22.04 sh -euc '
      export DEBIAN_FRONTEND=noninteractive PATH=/root/.cargo/bin:$PATH
      apt-get update -qq && apt-get install -y -qq build-essential curl ca-certificates >/dev/null
      command -v cargo >/dev/null || curl -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal
      rustup update stable >/dev/null
      cargo build --release --target-dir target/compat -p broche-gui -p broche-cli'
  NAME="$NAME-compat"
  STAGE="$OUT/$NAME"
  rm -rf "$STAGE"; mkdir -p "$STAGE/bin"
else
  BUILT=target/release
  cargo build --release -p broche-gui -p broche-cli
fi
install -m755 "$BUILT/broche" "$STAGE/bin/broche"
install -m755 "$BUILT/broche-cli" "$STAGE/bin/broche-cli"
D=apps/linux/data
install -Dm644 "$D/$ID.desktop" "$STAGE/share/applications/$ID.desktop"
install -Dm644 "$D/$ID.metainfo.xml" "$STAGE/share/metainfo/$ID.metainfo.xml"
install -Dm644 "$D/icon-256.png" "$STAGE/share/icons/hicolor/256x256/apps/$ID.png"
install -Dm644 "$D/icon-512.png" "$STAGE/share/icons/hicolor/512x512/apps/$ID.png"
install -Dm644 "$D/$ID.svg" "$STAGE/share/icons/hicolor/scalable/apps/$ID.svg"
install -Dm644 LICENSES/egui-default-fonts.txt "$STAGE/share/licenses/$ID/egui-default-fonts.txt"
install -m755 apps/linux/packaging/install.sh "$STAGE/install.sh"
install -Dm644 LICENSE "$STAGE/LICENSE"
mkdir -p "$STAGE/LICENSES"
cp -R LICENSES/. "$STAGE/LICENSES/"
install -Dm644 TRADEMARKS.md "$STAGE/TRADEMARKS.md"
install -Dm644 apps/linux/README.md "$STAGE/README.md"
tar -C "$OUT" -czf "$OUT/$NAME.tar.gz" "$NAME"
echo "tarball: $OUT/$NAME.tar.gz"

[ "${APPIMAGE:-1}" = 1 ] || exit 0
TOOL="$(command -v appimagetool || true)"
if [ -z "$TOOL" ]; then
  echo "appimagetool no está instalado; se conserva el tar.gz. No se descargan ni ejecutan binarios automáticamente."
  exit 0
fi
APPDIR="$OUT/Broche.AppDir"
rm -rf "$APPDIR"; mkdir -p "$APPDIR/usr"
cp -r "$STAGE/bin" "$STAGE/share" "$APPDIR/usr/"
cp "$STAGE/share/applications/$ID.desktop" "$APPDIR/$ID.desktop"
cp "$STAGE/share/icons/hicolor/256x256/apps/$ID.png" "$APPDIR/$ID.png"
cat > "$APPDIR/AppRun" <<'RUN'
#!/bin/sh
HERE="$(dirname "$(readlink -f "$0")")"
export PATH="$HERE/usr/bin:$PATH"
exec "$HERE/usr/bin/broche" "$@"
RUN
chmod +x "$APPDIR/AppRun"
# A runtime kept next to appimagetool (or APPIMAGE_RUNTIME) saves a download on every build.
RUNTIME="${APPIMAGE_RUNTIME:-$(dirname "$(readlink -f "$TOOL")")/runtime-$ARCH}"
set --
[ -f "$RUNTIME" ] && set -- --runtime-file "$RUNTIME"
if ARCH="$ARCH" "$TOOL" --appimage-extract-and-run "$@" "$APPDIR" "$OUT/$NAME.AppImage" >"$OUT/appimagetool.log" 2>&1; then
  echo "AppImage: $OUT/$NAME.AppImage"
else
  echo "the AppImage could not be built; see $OUT/appimagetool.log" >&2
  exit 1
fi
