# Contributing to Broche

Thank you for helping with Broche. The project is young, so the codebase
and these guidelines still change often.

## Before you start

- Read [docs/technical/](docs/technical/README.md) for the architecture, and the
  open issues for what is planned.
- For anything bigger than a small fix, open an issue first so we can agree on
  the approach.
- Be kind. Everyone is expected to follow the [Code of Conduct](CODE_OF_CONDUCT.md).

## Sign your commits (DCO)

Broche uses the [Developer Certificate of Origin](DCO.txt) instead of a
CLA. By signing off, you certify that you wrote the change or otherwise have
the right to submit it under the project's licenses.

Add a sign-off line to every commit:

```
git commit -s -m "Your message"
```

This appends a line like:

```
Signed-off-by: Your Name <you@example.com>
```

Commits without a sign-off can't be merged.

## Licensing of contributions

Your contribution is licensed under the license that covers the files you
change, as described in the top-level [LICENSE](LICENSE) file. For GPL-licensed
parts, this includes the [app-store exception](LICENSES/APP-STORE-EXCEPTION.md).

## Building and checking

The [README](README.md#build-from-source-on-linux) lists the requirements and build commands.
Before opening a pull request, run the same checks as CI:

```sh
cargo fmt --all --check
cargo clippy --locked -p broche-protocol -p broche-core -p broche-ffi -p broche-cli -p broche-gui --all-targets -- -D warnings
cargo test --locked -p broche-protocol -p broche-core -p broche-ffi -p broche-cli -p broche-gui
(cd apps/android && ./gradlew testDebugUnitTest lintDebug assembleDebug)
```

## What gets published

`.gitignore` is an allow-list: only the paths it names reach the repository.
Ask before adding one. `scripts/check-public.sh` refuses private keys, tokens,
this machine's network addresses and the private words listed in
`~/.config/broche-dev/private-words` (your name, e-mail, home folder; that file
never enters the repository). Run it before every commit, or let Git run it:

```sh
ln -s ../../scripts/check-public.sh .git/hooks/pre-commit
```

## Ground rules for code

These come from the plan and will grow into a full style guide:

- **Rust is the source of truth.** Business logic lives in the shared core;
  the Linux app (egui) and Android (Compose) only describe screens.
- **One bridge on Android.** Only `crates/broche-ffi` (UniFFI) connects the core to Kotlin.
- **Privacy first.** Never log notification text, message content, file names
  or keys. No telemetry.
- **All user-facing strings are translatable**, in English and Spanish
  (`apps/linux/src/i18n.rs`, string resources on Android).
- **Accessibility is not optional.** Labels for screen readers and full
  keyboard navigation from day one.
- Format with `cargo fmt` and keep `cargo clippy` clean. Android code follows
  the official Kotlin style.

## Reporting bugs

Open an issue with your Linux distribution, phone model, Android version, and the
Broche versions on both sides. Security problems go through
[SECURITY.md](SECURITY.md) instead, never a public issue.
