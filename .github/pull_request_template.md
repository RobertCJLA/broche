<!-- Pull requests for Broche are written in English. -->

## What and why

<!-- What this change does and the problem it solves. Link the issue if there is one. -->

## How it was tested

<!-- Tests added or run, and manual checks (Linux desktop/session, phone model, Android version). -->

## Checklist

- [ ] CI passes for the parts I changed (`cargo test`, `cargo clippy`, `./gradlew testDebugUnitTest lintDebug`)
- [ ] User-facing texts exist in **English and Spanish** (Android `values` + `values-es`; Linux `i18n.rs`)
- [ ] New UI works with keyboard and screen readers
- [ ] No personal content (notification text, messages, file names, keys) is logged
- [ ] `CHANGELOG.md` (`## Unreleased`) and the affected docs (README, `docs/`) are updated
