# Releasing

Versions follow [docs/VERSIONING.md](../VERSIONING.md): `scripts/bump-version.sh X.Y.Z`,
an annotated tag `vX.Y.Z`, and the `## Unreleased` section of `CHANGELOG.md`.
Until 1.0, releases are marked **Pre-release** on GitHub.

## What a release is

| File | Built by |
|---|---|
| `Broche-<version>-x86_64.flatpak` | `flatpak-builder --repo` and `flatpak build-bundle` (`apps/linux/packaging/flatpak/`) |
| `Broche-<version>-x86_64.AppImage` | `COMPAT=1 scripts/package-linux.sh` (needs `appimagetool`) |
| `Broche-<version>-linux-x86_64.tar.gz` | `COMPAT=1 scripts/package-linux.sh`: built in an Ubuntu 22.04 container, runs on glibc 2.35 and later |
| `Broche-<version>-android.apk` | `./gradlew assembleRelease` for `arm64-v8a` and `armeabi-v7a`, signed with the release key |
| `SHA256SUMS.txt` | `sha256sum` of the four files |

`scripts/release-local.sh <version>` checks first that podman, appimagetool, flatpak-builder
and the Rust targets `aarch64-linux-android` and `armv7-linux-androideabi` are there, then builds
all of them into `target/release-files/`
(it renames the `…-compat` tarball and AppImage that `COMPAT=1` makes).
Then:

```sh
gh release create vX.Y.Z --prerelease --title "Broche X.Y.Z (beta)" \
  --notes-file notes.md target/release-files/*
```

`.github/workflows/release.yml`, run by hand on the tag (Actions → Release → Run
workflow), builds only the tarball and the APK and drafts a release with them; the
Flatpak and the AppImage come from `release-local.sh` for now.

## Signing the Android app

The release key is a Java keystore with the alias `broche`. Gradle reads it from
`BROCHE_KEYSTORE` (the file) and `BROCHE_KEYSTORE_PASSWORD`; without them the
release build falls back to the debug key, which can't update an installed release.
In GitHub Actions the keystore comes base64-encoded from the `ANDROID_KEYSTORE` secret.

The key must never be lost or replaced: Android only installs an update signed
with the same key. Keep a backup outside the repository.

## Updates

Broche has no updater: NectarLink's self-updaters were removed. Install a new
version over the old one (Flatpak bundle, AppImage, `install.sh`, or the APK).
