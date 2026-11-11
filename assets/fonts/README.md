# Fonts

The typefaces both apps ship with, as named in `docs/design/tokens.json`.
All are under the [SIL Open Font License 1.1](https://openfontlicense.org);
each license sits next to its font and is packaged with it.

| File | Family | Used for |
|---|---|---|
| `Figtree.ttf` | Figtree (variable, weights 300 to 900) | All text in Bloom; body text in Graphite |
| `SpaceMono-Regular.ttf`, `SpaceMono-Bold.ttf` | Space Mono | Codes, IDs and Graphite labels |

Taken unmodified from [google/fonts](https://github.com/google/fonts) at
commit `6e8069ff8ba3dab2a397fb30e7fbd243aba9b57a` (`ofl/figtree`,
`ofl/spacemono`).

The Android app packages this folder as assets (`apps/android/app/build.gradle.kts`).
