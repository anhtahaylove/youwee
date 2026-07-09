# Youwee Custom 0.19.1-custom.12

Windows x64 installer:
https://github.com/anhtahaylove/youwee-releases/releases/download/v0.19.1-custom.12/Youwee_0.19.1-custom.12_x64-setup.exe

Release page:
https://github.com/anhtahaylove/youwee-releases/releases/tag/v0.19.1-custom.12

SHA256:
`b83280e8a4c818b77a97db2ab855ad469243411bcc48eecbb2c06e264c7abc7d`

## What changed

- Updated Tauri build tooling and compatible dependencies from upstream PR #103.
- Keeps Linux AppImage desktop icon metadata aligned with the newer build stack.
- No runtime download behavior was intentionally changed in this release.

## What to test

- Install and launch the app; version should show `0.19.1-custom.12`.
- From `0.19.1-custom.11`, click `Update Now`; the app should update and relaunch as `0.19.1-custom.12`.
- Confirm updater metadata uses `anhtahaylove/youwee-releases`, not upstream.
- Smoke test one normal YouTube download and one Universal input paste.

## Known gap

- This release was verified on the current Windows machine. A clean Windows VM remains the best final install check.
