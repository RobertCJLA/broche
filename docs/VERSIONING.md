# Versioning and change history

How Broche keeps track of versions, branches and documentation. Pull
requests, commit messages for `main`, CHANGELOG.md and this file are written
in **English**.

## Version numbers

- One version for the whole project, in `[workspace.package] version` of the
  root `Cargo.toml`. The Linux app, the CLI and the Android app all read it
  (Android derives `versionCode` from it: 1.2.3 → 1002003, so every release
  must be larger than the last).
- [Semantic versioning](https://semver.org): `MAJOR.MINOR.PATCH`. Before 1.0,
  a **minor** bump may need a step from the user (re-pairing, a changed data
  folder, a new permission) and its CHANGELOG entry says so; a **patch** is
  fixes only.
- Broche starts from NectarLink 0.1.4. Its first own release is **0.2.0**
  (the Linux app and the removed features are a minor-level change).
- Bump with `scripts/bump-version.sh <new-version>`: it updates Cargo.toml
  (the workspace version and the internal crate references), Cargo.lock, the
  AppStream metadata of the Linux app and moves the `Unreleased` notes in
  CHANGELOG.md under the new version with today's date.
  Given the version `Cargo.toml` already has and no tag for it yet (0.2.0 was
  set ahead of time), it only folds the `Unreleased` notes into that
  version's section (after its introduction and summary, before its older
  `###` notes) and dates it, and the AppStream release, today; once
  `vX.Y.Z` is tagged it refuses.

## Branches and pull requests

- `main` is always buildable; work happens on feature branches and reaches
  `main` through a pull request (title and description in English, filled
  from `.github/pull_request_template.md`).
- Each pull request that changes behaviour adds a line under `## Unreleased`
  in CHANGELOG.md and updates the documentation it affects (README, docs/). Documentation is versioned with the
  code: no separate docs branch.

## Releases

1. Merge everything for the release into `main`; CI must be green.
2. `scripts/bump-version.sh 0.2.0`, review the diff, commit
   (`Release 0.2.0`) and merge it.
3. Tag the merge commit: `git tag -a v0.2.0 -m "Broche 0.2.0"` and push the
   tag. Tags are annotated and never moved or reused.
4. Build the release files and publish them as a GitHub **Pre-release**
   (before 1.0): see [technical/releasing.md](technical/releasing.md).
