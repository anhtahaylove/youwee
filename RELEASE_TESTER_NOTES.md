# Youwee Custom 0.19.1-custom.17

Windows x64 installer:
https://github.com/anhtahaylove/youwee/releases/download/v0.19.1-custom.17/Youwee-Windows-Setup.exe

Release page:
https://github.com/anhtahaylove/youwee/releases/tag/v0.19.1-custom.17

SHA256:
`c60af3ac470d290a94ae3f201261104c5e5e1752c93290bf096146e0694c0e9f`

## What changed

- TikTok Live Telegram `/tl_*` commands now run in the Rust backend.
- `/tl_status` and `/tl_watchlist` work while Youwee is hidden or minimized.
- Updater metadata stays on the custom `anhtahaylove/youwee` release channel.

## What to test

- Install and launch the app; version should show `0.19.1-custom.17`.
- From an older custom build, click `Update Now`; the app should update to `0.19.1-custom.17`.
- In the Telegram Topic, send `/tl_status` and `/tl_watchlist`; the bot should reply.
- Add a TikTok Live target with `/tl_add @username`, then confirm it appears in the TikTok Live watchlist.

## Known gap

- Bot reply text is visible in Telegram, not stored in the local app database.
