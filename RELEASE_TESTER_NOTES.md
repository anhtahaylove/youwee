# Youwee Custom 0.19.1-custom.4

Windows x64 installer:
https://github.com/anhtahaylove/youwee-releases/releases/download/v0.19.1-custom.4/Youwee_0.19.1-custom.4_x64-setup.exe

Release page:
https://github.com/anhtahaylove/youwee-releases/releases/tag/v0.19.1-custom.4

SHA256:
`5fc09e19933d33e3151938818b3b6b03d00a026a895f50498d10c46c6e237f4b`

## What to test

- Install and launch the app; version should show `0.19.1-custom.4`.
- Paste text that contains a URL into Download, Universal Download, Gallery, and Metadata inputs; the app should extract the URL.
- Use Show in folder on a downloaded item; Windows should open the file location.
- Check updater metadata; it should use `anhtahaylove/youwee-releases`, not upstream.
- Confirm the browser extension still shows the custom `anhtahaylove` branding.

## Known gap

- This release was verified on the current Windows machine. A separate clean Windows VM is still the best final install check.
