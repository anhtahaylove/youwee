# Youwee Browser Extension Store Listing Kit

This document is the source of truth for the Firefox Add-ons (AMO) and Chrome Web Store listings.

## Current distribution status

The Firefox add-on is **public and listed on AMO** at <https://addons.mozilla.org/firefox/addon/youwee-download-companion/>. AMO approved version `0.19.1.37`, which is now the canonical stable Firefox channel.

The Firefox source manifest intentionally omits `update_url`, so Firefox receives future stable updates from AMO. GitHub releases retain a validated one-time `firefox-updates.json` bridge that moves older self-hosted installs to the signed AMO `0.19.1.37` XPI.

Use `Youwee-Extension-Firefox-AMO.zip` only when submitting a new, unique listed version. Do not publish new unlisted stable builds or reuse a version that already exists on AMO.

## Shared product identity

| Field | Value |
| --- | --- |
| Product name | Youwee Download Companion |
| Suggested AMO slug | `youwee-download-companion` |
| Publisher / owner | `anhtahaylove` |
| Original project credit | Youwee was created by `vanloctech`; this fork and extension distribution are maintained by `anhtahaylove`. |
| License | MIT |
| Homepage | <https://github.com/anhtahaylove/youwee> |
| Support | <https://github.com/anhtahaylove/youwee/issues> |
| Privacy policy | <https://github.com/anhtahaylove/youwee/blob/main/docs/EXTENSION_PRIVACY.md> |
| Required companion | Free Youwee desktop app |
| Firefox categories | Download Management; Photos, Music & Videos |
| Chrome category | Productivity |
| Mature content | No |
| Paid features | No |

Only download media that you own or are authorized to save. Users remain responsible for copyright, website terms, and local law. The extension does not bypass DRM.

## English (`en-US`)

### Name

Youwee Download Companion

### Summary

Send the current media page to Youwee, choose video or audio quality, and start or queue a download.

### Description

Youwee Download Companion sends the media page you are viewing to the free Youwee desktop application.

Use the popup or the optional floating button to:

- Send a page to Youwee with one click.
- Choose video or audio and the preferred quality.
- Start immediately or add the item to the queue.
- Open supported YouTube videos in AI Summary.
- Keep YouTube watch links focused on the current video instead of accidentally adding a playlist.

The floating button is available on supported media sites, including YouTube, TikTok, Instagram, Facebook, X/Twitter, Vimeo, Twitch, Bilibili, Dailymotion, and SoundCloud. The popup can send any valid HTTP or HTTPS page to Youwee Universal Download.

The free Youwee desktop app is required. Install and open Youwee once before using the extension so the operating system can register the `youwee://` protocol.

Privacy: the extension has no ads, analytics, or tracking. It sends the active page URL and your selected download options only to the Youwee app installed on the same computer, and only after you click an action.

Please download only content you own or have permission to save. This extension does not bypass DRM.

## Vietnamese (`vi`)

### Tên

Youwee - Tiện ích tải media

### Tóm tắt

Gửi trang media hiện tại sang Youwee, chọn video hoặc âm thanh và chất lượng, rồi tải ngay hoặc thêm vào hàng đợi.

### Mô tả

Youwee - Tiện ích tải media giúp gửi trang bạn đang xem sang ứng dụng desktop Youwee miễn phí.

Dùng popup hoặc nút nổi tùy chọn để:

- Gửi trang hiện tại sang Youwee bằng một lần bấm.
- Chọn video hoặc âm thanh và chất lượng mong muốn.
- Tải ngay hoặc thêm nội dung vào hàng đợi.
- Mở video YouTube được hỗ trợ trong Tóm tắt AI.
- Giữ link YouTube ở đúng video hiện tại, tránh vô tình thêm cả playlist.

Nút nổi hoạt động trên các website media được hỗ trợ như YouTube, TikTok, Instagram, Facebook, X/Twitter, Vimeo, Twitch, Bilibili, Dailymotion và SoundCloud. Popup có thể gửi mọi trang HTTP hoặc HTTPS hợp lệ sang Universal Download của Youwee.

Cần cài ứng dụng desktop Youwee miễn phí. Hãy cài và mở Youwee ít nhất một lần để hệ điều hành đăng ký giao thức `youwee://`.

Quyền riêng tư: tiện ích không có quảng cáo, analytics hoặc tracking. URL trang đang mở và lựa chọn tải chỉ được gửi tới ứng dụng Youwee trên cùng máy sau khi bạn chủ động bấm thao tác.

Chỉ tải nội dung bạn sở hữu hoặc được phép lưu. Tiện ích không vượt qua DRM.

## Simplified Chinese (`zh-CN`)

### 名称

Youwee 下载助手

### 摘要

将当前媒体页面发送到 Youwee，选择视频或音频及画质，然后立即下载或加入队列。

### 描述

Youwee 下载助手可将当前媒体页面发送到免费的 Youwee 桌面应用。

使用弹出窗口或可选悬浮按钮可以：

- 一键将当前页面发送到 Youwee。
- 选择视频或音频及所需画质。
- 立即开始下载或加入队列。
- 在 AI Summary 中打开受支持的 YouTube 视频。
- 规范化 YouTube 观看链接，避免误将整个播放列表加入队列。

悬浮按钮支持 YouTube、TikTok、Instagram、Facebook、X/Twitter、Vimeo、Twitch、Bilibili、Dailymotion 和 SoundCloud。弹出窗口可以将任何有效的 HTTP 或 HTTPS 页面发送到 Youwee Universal Download。

需要安装免费的 Youwee 桌面应用。请先安装并至少启动一次 Youwee，以便操作系统注册 `youwee://` 协议。

隐私：扩展不包含广告、分析或跟踪。仅在用户主动点击操作后，才会把当前页面 URL 和下载选项发送到同一台电脑上的 Youwee 应用。

请只下载您拥有或获准保存的内容。本扩展不会绕过 DRM。

## AMO additional details

### Authors and license

- Owner and listed maintainer: `anhtahaylove`.
- License: MIT.
- Credit the original Youwee project and `vanloctech` in the description or developer profile.
- Do not add another AMO author unless that person has agreed and has an AMO account.

### Technical details

- Manifest V3.
- Firefox Desktop 140 or later.
- Not offered for Firefox for Android because the companion desktop application and `youwee://` handler are required.
- No remote executable code, minification, obfuscation, analytics, advertisements, or tracking.
- Source code: <https://github.com/anhtahaylove/youwee/tree/main/extensions/youwee-webext>.
- Build commands: `bun install --frozen-lockfile`, then `bun run ext:package`.
- Public AMO package: `extensions/youwee-webext/dist/packages/Youwee-Extension-Firefox-AMO.zip`.

### Reviewer notes / test instructions

1. Install the free Youwee desktop app from the repository release page and launch it once so Windows registers `youwee://`.
2. Install the extension package and open a supported public media page.
3. Open the extension popup. Confirm the page URL is shown.
4. Choose Video or Audio and a quality, then click **Add to queue**.
5. Accept the browser prompt to open Youwee. The desktop app should open the matching YouTube or Universal page and add the URL to its queue.
6. Enable or disable the floating button in the popup and verify the change on a supported page.
7. On a YouTube watch page, verify **AI Summary** opens the same video in Youwee's AI Summary page.

No test account or credentials are required. The extension never accesses cookies. All JavaScript is readable and packaged with the submission.

### Data disclosure

- Firefox built-in consent type: `browsingActivity` (required for the user-initiated current-page URL transfer to the local desktop app).
- The extension processes the current URL and media-player layout locally.
- The active URL and selected options are transferred only after an explicit user action.
- No data is sent to developer-operated analytics, advertising, or tracking servers.
- Locally stored data is limited to floating-button interface preferences.

## Chrome Web Store privacy fields

### Single purpose

Send the current media page and user-selected download settings to the locally installed Youwee desktop application.

### Permission justifications

- `activeTab`: access the active page URL only after the user invokes the extension.
- `storage`: store floating-button preferences locally in the browser.
- `scripting`: restore packaged content scripts and CSS on a supported tab that was already open when the extension was installed or reloaded.
- Host access: show the optional floating button and process the current URL and media-player layout locally on the explicitly listed supported websites.

### Remote code

Select **No, I am not using remote code**. All JavaScript and CSS executed by the extension are included in the uploaded package.

### User data

Disclose **Web history** for the active page URL and **Website content** for the locally inspected media-player layout. Explain that page layout is not retained, and the URL is transferred only to the local Youwee app after an explicit click. Certify the Chrome Web Store Limited Use statements.

Use the privacy policy URL listed in the shared product identity table.

## Store graphics checklist

Existing source asset:

- `extensions/youwee-webext/src/icons/logo-128.png` - 128x128 store icon.
- `docs/screenshots/youwee-extension-chrome-firefox.png` - existing product overview; suitable as source material but not the exact Chrome screenshot size.

Ready-to-upload shared assets are in `docs/store-assets/`:

- `screenshot-popup-1280x800.png` - popup controls on a public example video.
- `screenshot-desktop-queue-1280x800.png` - local desktop queue handoff.
- `promo-small-440x280.png` - Chrome small promotional tile.
- `promo-marquee-1400x560.png` - Chrome marquee promotional tile.

Chrome requires at least one 1280x800 screenshot and a 440x280 small promotional tile. The 1400x560 marquee tile is optional. The current files contain no personal tabs, profile names, download paths, tokens, or private URLs.

## Public AMO publication sequence

1. Bump the independent extension version in both source manifests; do not couple it to the desktop app version.
2. Run `bun run ext:package` and validate `Youwee-Extension-Firefox-AMO.zip`.
3. In **Manage Status & Versions**, upload that package as a new **listed / On this site** version for Firefox Desktop.
4. Add reviewer notes, submit for review, and wait for AMO approval before calling the version stable.
5. After approval, update the AMO migration bridge only when older self-hosted users need a newer handoff target.
6. Keep the add-on ID `youwee@anhtahaylove.com`; changing it would create a separate extension and lose the migration path.

## Chrome Web Store publication sequence

1. Register a Chrome Web Store developer account, enable two-step verification, and pay the one-time registration fee.
2. Run `bun run ext:package` and upload `Youwee-Extension-Chromium.zip`.
3. Fill Store Listing, Privacy, Distribution, and Test Instructions with the copy above.
4. Upload the required graphics, choose public visibility, and submit for review.
5. After approval, use the Chrome Web Store build as the canonical stable Chromium package; keep **Load unpacked** only for development and testing.
