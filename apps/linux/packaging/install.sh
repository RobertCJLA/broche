#!/bin/sh
# Installs Broche for Linux under a prefix (default: ~/.local).
#   ./install.sh [--prefix DIR]        install
#   ./install.sh [--prefix DIR] --uninstall
# Run it from the folder that holds bin/ and share/ (the release tarball), or
# from the repository after `cargo build --release`.
set -eu
PREFIX="${HOME}/.local"
UNINSTALL=0
while [ $# -gt 0 ]; do
  case "$1" in
    --prefix) PREFIX="$2"; shift 2 ;;
    --uninstall) UNINSTALL=1; shift ;;
    *) echo "unknown option: $1" >&2; exit 2 ;;
  esac
done
HERE="$(cd "$(dirname "$0")" && pwd)"
ID=io.github.robertcjla.Broche
# Removes one build's program and menu files; never the user's data.
remove() { # $1: app ID, then its binaries
  id="$1"; shift
  for b in "$@"; do rm -f "$PREFIX/bin/$b"; done
  rm -f "$PREFIX/share/applications/$id.desktop" \
        "$PREFIX/share/metainfo/$id.metainfo.xml" \
        "$PREFIX/share/licenses/$id/egui-default-fonts.txt" \
        "$PREFIX/share/icons/hicolor/256x256/apps/$id.png" \
        "$PREFIX/share/icons/hicolor/512x512/apps/$id.png" \
        "$PREFIX/share/icons/hicolor/scalable/apps/$id.svg"
}
if [ "$UNINSTALL" = 1 ]; then
  remove "$ID" broche broche-cli
  # Its autostart entry would start a program that is gone.
  rm -f "${XDG_CONFIG_HOME:-$HOME/.config}/autostart/broche.desktop"
  echo "Broche removed from $PREFIX"
  exit 0
fi
BIN="$HERE/bin"; DATA="$HERE/share"
[ -x "$BIN/broche" ] || { BIN="$HERE/../../../target/release"; DATA="$HERE/../data"; }
[ -x "$BIN/broche" ] || { echo "broche not found: build it first" >&2; exit 1; }
install -Dm755 "$BIN/broche" "$PREFIX/bin/broche"
[ -x "$BIN/broche-cli" ] && install -Dm755 "$BIN/broche-cli" "$PREFIX/bin/broche-cli"
if [ -d "$HERE/share" ]; then
  cp -r "$HERE/share/." "$PREFIX/share/"
else
  install -Dm644 "$DATA/$ID.desktop" "$PREFIX/share/applications/$ID.desktop"
  install -Dm644 "$DATA/$ID.metainfo.xml" "$PREFIX/share/metainfo/$ID.metainfo.xml"
  install -Dm644 "$DATA/icon-256.png" "$PREFIX/share/icons/hicolor/256x256/apps/$ID.png"
  install -Dm644 "$DATA/icon-512.png" "$PREFIX/share/icons/hicolor/512x512/apps/$ID.png"
  install -Dm644 "$DATA/$ID.svg" "$PREFIX/share/icons/hicolor/scalable/apps/$ID.svg"
  install -Dm644 "$HERE/../../../LICENSES/egui-default-fonts.txt" "$PREFIX/share/licenses/$ID/egui-default-fonts.txt"
fi
# Full path to the program: GNOME Shell's PATH often lacks ~/.local/bin, and then it ignores
# the menu entry, so the window and its notifications show a bare ID and a generic icon.
sed -i "s|^Exec=broche |Exec=$PREFIX/bin/broche |" "$PREFIX/share/applications/$ID.desktop"
command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database "$PREFIX/share/applications" 2>/dev/null || true
command -v gtk-update-icon-cache >/dev/null 2>&1 && gtk-update-icon-cache -q -t "$PREFIX/share/icons/hicolor" 2>/dev/null || true
echo "Broche installed in $PREFIX (bin/broche)."
echo "Make sure $PREFIX/bin is in your PATH."
