# Youwee Custom 0.19.1-custom.13

Windows x64 installer:
https://github.com/anhtahaylove/youwee/releases/download/v0.19.1-custom.13/Youwee-Windows-Setup.exe

Release page:
https://github.com/anhtahaylove/youwee/releases/tag/v0.19.1-custom.13

SHA256:
`59508de1ea79323fb316111ec8501934298a70b00968eef2690bfbf4e6973f51`

## What changed

- Moved updater metadata, Windows installer downloads, and extension download links to the main `anhtahaylove/youwee` release page.
- Keeps the old `youwee-releases` repo available only as a transition channel for older installed builds.
- No runtime download behavior was intentionally changed in this release.

## What to test

- Install and launch the app; version should show `0.19.1-custom.13`.
- From `0.19.1-custom.12`, click `Update Now`; the app should update and relaunch as `0.19.1-custom.13`.
- Confirm updater metadata uses `anhtahaylove/youwee`, not upstream or `youwee-releases`.
- Smoke test one normal YouTube download and one Universal input paste.

## Known gap

- This release was verified on the current Windows machine. A clean Windows VM remains the best final install check.
