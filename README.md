# Youwee Custom Fork

<div align="center">

  [![English](https://img.shields.io/badge/lang-English-blue)](README.md)
  [![Tiếng Việt](https://img.shields.io/badge/lang-Tiếng_Việt-red)](docs/README.vi.md)
  [![简体中文](https://img.shields.io/badge/lang-简体中文-green)](docs/README.zh-CN.md)
  ![Français](https://img.shields.io/badge/lang-Français-0055A4)
  ![Русский](https://img.shields.io/badge/lang-Русский-1F5FBF)
  ![العربية](https://img.shields.io/badge/lang-%D8%A7%D9%84%D8%B9%D8%B1%D8%A8%D9%8A%D8%A9-0A8F6A)
  ![ไทย](https://img.shields.io/badge/lang-%E0%B9%84%E0%B8%97%E0%B8%A2-7B1FA2)
  ![Português](https://img.shields.io/badge/lang-Português-009C3B)
  [![Upstream](https://img.shields.io/badge/upstream-vanloctech%2Fyouwee-64748b?logo=github)](https://github.com/vanloctech/youwee)

  <img src="src-tauri/icons/icon.png" alt="Youwee Logo" width="128" height="128">
  
  **Personal Windows-focused Youwee fork with upstream tracking, custom downloader fixes, and signed custom releases**

  [![Custom Releases](https://img.shields.io/github/downloads/anhtahaylove/youwee/total?label=Custom%20Downloads)](https://github.com/anhtahaylove/youwee/releases)
  [![Source Fork](https://img.shields.io/badge/source-anhtahaylove%2Fyouwee-0EA5E9?logo=github)](https://github.com/anhtahaylove/youwee)
  [![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](https://opensource.org/licenses/MIT)
  [![Reddit](https://img.shields.io/badge/Reddit-r%2Fyouwee-FF4500?logo=reddit&logoColor=white)](https://www.reddit.com/r/youwee)
  [![Website](https://img.shields.io/badge/Website-youwee.app-0EA5E9)](https://youwee.app)
  [![Discord](https://img.shields.io/badge/Discord-Community-5865F2?logo=discord&logoColor=white)](https://discord.gg/yCrs9hcw)

<a href="https://www.producthunt.com/products/youwee/reviews/new?utm_source=badge-product_review&utm_medium=badge&utm_source=badge-youwee" target="_blank"><img src="https://api.producthunt.com/widgets/embed-image/v1/product_review.svg?product_id=1154224&theme=light" alt="Youwee - A modern YouTube downloader @yt-dlp GUI for cross-platform | Product Hunt" width="250" height="54"></a>
</div>

---

## About This Fork

This repository is the `anhtahaylove/youwee` custom fork of [vanloctech/youwee](https://github.com/vanloctech/youwee). It stays close to upstream, but carries Windows-first fixes and experiments that are useful for daily use before they are accepted upstream.

Custom release binaries now live on the same [anhtahaylove/youwee](https://github.com/anhtahaylove/youwee/releases) fork, so source, issues, releases, and updater metadata share one canonical repo.

### Custom Highlights

- **Custom updater channel** — Uses `anhtahaylove/youwee` instead of upstream release prompts.
- **Facebook Reels core fallback** — Keeps Reels downloads inside the core download path with better Library/history metadata.
- **Unicode-safe download titles on Windows** — Preserves Vietnamese and other Unicode titles in Logs and Library when yt-dlp stdout loses characters.
- **Browser extension branding** — Chromium/Firefox extension UI uses the custom fork identity and keeps unsupported actions hidden.
- **Telegram Remote Download topics** — Supports `message_thread_id` for Telegram forum topics.
- **Firefox profile folder resolution** — Accepts real Firefox profile folder names such as `i879pxds.default-release`.
- **Upstream v0.19 feature ports** — Includes selected Library, media split, collection organization, queue output folder, numbering, and UI fixes.

## Features

- **Video Downloads** — YouTube, TikTok, Facebook, Instagram, Bilibili, Youku, and 1800+ sites
- **Browser Extension Bridge** — Chromium + Firefox extension with floating button, media/quality picker, and one-click `Download now` / `Add to queue` send to Youwee app
- **Plugins & Workflow Automation** — Install signed plugins, configure custom fields, assign them to download workflows, and extend Youwee with notifications, uploads, and post-download automations
- **Channel Follow** — Follow YouTube, Bilibili & Youku channels, get notified of new videos, auto-download, and manage from system tray
- **Metadata Fetcher** — Download video info, descriptions, comments, and thumbnails without the video
- **Live Stream Support** — Download live streams with dedicated toggle
- **AI Video Summary** — Summarize videos with Gemini, OpenAI, or Ollama
- **AI Video Processing** — Edit videos using natural language (cut, convert, resize, extract audio)
- **Time Range Download (Cut Video)** — Download only the segment you need by setting start/end time
- **Batch & Playlist** — Download multiple videos or entire playlists
- **Audio Extraction** — Extract audio in MP3, M4A, or Opus formats
- **Subtitle Support** — Download or embed subtitles
- **Subtitle Workshop** — Create, edit, and refine subtitles (SRT/VTT/ASS) with timing tools, find/replace, auto-fix, AI Translate, AI Grammar Fix, and Whisper generation
- **Subtitle Page Core Features** — Waveform/spectrogram timeline, shot-change sync, realtime QC with style profiles, split/merge tools, translator mode (source/target), and batch/project operations
- **Post-Processing** — Auto-embed metadata, thumbnail, and subtitles (when enabled) into output files
- **SponsorBlock** — Automatically skip sponsors, intros, outros, and self-promotions with remove/mark/custom modes
- **Speed Limit** — Control download bandwidth (KB/s, MB/s, GB/s)
- **Download Library** — Track and manage all your downloads
- **6 Beautiful Themes** — Midnight, Aurora, Sunset, Ocean, Forest, Candy
- **Fast & Lightweight** — Designed for minimal resource usage

## Screenshots
![Youwee](docs/screenshots/youwee-youtube.png)

<details>
<summary><strong>More Screenshots</strong></summary>

![Youwee - Universal](docs/screenshots/youwee-universal.png)
![Youwee - Gallery](docs/screenshots/youwee-gallery.png)
![Youwee - Channels](docs/screenshots/youwee-channels.png)
![Youwee - AI Summary](docs/screenshots/youwee-ai-summary.png)
![Youwee - Processing 1](docs/screenshots/youwee-processing.png)
![Youwee - Processing 2](docs/screenshots/youwee-processing-2.png)
![Youwee - Subtitles](docs/screenshots/youwee-subtitles.png)
![Youwee - Metadata](docs/screenshots/youwee-metadata.png)
![Youwee - Library](docs/screenshots/youwee-library.png)
![Youwee - Logs](docs/screenshots/youwee-logs.png)
![Youwee - Setting - General](docs/screenshots/youwee-setting-general.png)
![Youwee - Setting - Dependencies](docs/screenshots/youwee-setting-dependencies.png)
![Youwee - Setting - Download](docs/screenshots/youwee-setting-download.png)
![Youwee - Setting - AI Features](docs/screenshots/youwee-setting-ai-features.png)
![Youwee - Setting - Network & Auth](docs/screenshots/youwee-setting-network-auth.png)
![Youwee - Setting - Plugin](docs/screenshots/youwee-setting-plugins.png)
![Youwee - Setting - Remote Download](docs/screenshots/youwee-setting-remote-download.png)
![Youwee - Setting - Extension](docs/screenshots/youwee-setting-extension.png)
![Youwee - Setting - About](docs/screenshots/youwee-setting-about.png)
![Youwee - Browser Extension](docs/screenshots/youwee-extension-chrome-firefox.png)

</details>

## Demo Video

▶️ [Watch on YouTube](https://youtu.be/7eaKOsFAP1s)

## Legal Notice

Youwee is a local utility for downloading and processing media from URLs provided by the user. It is not affiliated with YouTube or any other media platform.

Use Youwee only with content you own, have permission to use, or are legally allowed to access and store. Users are responsible for complying with applicable laws, platform terms, copyright rules, and any required permissions. The Youwee project and maintainers are not responsible for misuse of the app.

## Installation

### Download for your platform

| Platform | Download |
|----------|----------|
| **Windows** (x64) | [Full Setup `.exe` — recommended](https://github.com/anhtahaylove/youwee/releases/latest/download/Youwee-Windows-Full-Setup.exe) · [Full `.msi`](https://github.com/anhtahaylove/youwee/releases/latest/download/Youwee-Windows-Full.msi) |
| **macOS** (Apple Silicon: M1/M2/M3/M4 or newer) | [Download latest Apple Silicon DMG](https://github.com/anhtahaylove/youwee/releases/latest/download/Youwee-Mac-Apple-Silicon.dmg) |
| **macOS** (Intel) | [Download latest Intel DMG](https://github.com/anhtahaylove/youwee/releases/latest/download/Youwee-Mac-Intel.dmg) |
| **Linux** (x86_64/amd64) | [Debian/Ubuntu `.deb` — recommended](https://github.com/anhtahaylove/youwee/releases/latest/download/Youwee-Linux.deb) · [Portable `.AppImage`](https://github.com/anhtahaylove/youwee/releases/latest/download/Youwee-Linux.AppImage) |

> See all custom release assets at [anhtahaylove/youwee](https://github.com/anhtahaylove/youwee/releases). For official upstream releases, use [vanloctech/youwee](https://github.com/vanloctech/youwee/releases).

**Not sure which file to choose?**

- On a normal Windows 10/11 PC, choose **`Youwee-Windows-Full-Setup.exe`**.
- On a company-managed Windows PC where an administrator deploys software, choose **`Youwee-Windows-Full.msi`**.
- On Ubuntu, Linux Mint, Pop!_OS, Zorin OS, Debian, or another Debian-based distribution, choose **`Youwee-Linux.deb`**.
- On another 64-bit Intel/AMD Linux distribution, try **`Youwee-Linux.AppImage`**.
- Do not download `.sig`, `.json`, `.txt`, extension archives, or `.app.tar.gz` files unless you specifically need update verification or browser-extension recovery.

### Install on Windows

The Full Setup installer is recommended for most users. It includes yt-dlp, FFmpeg/ffprobe, Deno, gallery-dl, and Chromium/Firefox extension recovery packages, so a new user does not need to install command-line dependencies manually.

#### First installation

1. Download `Youwee-Windows-Full-Setup.exe` from the table above.
2. Open **Downloads** in File Explorer and double-click the downloaded file.
3. Approve the Windows permission prompt, then keep the default installation options unless you have a specific reason to change them.
4. Launch **Youwee** from the Start menu or desktop shortcut.
5. In Youwee, open **Settings → Dependencies**. The bundled tools should show their installed versions without requiring a separate download.
6. Optional: open **Settings → Browser Extension** for guided Firefox, Chrome, Edge, Brave, or other Chromium-browser setup.

The `.msi` package contains the same Full dependency pack and is intended for users or administrators who prefer Windows Installer deployment. Do not install both formats for the same version.

#### Upgrade an existing installation

1. Finish or cancel active downloads and recordings.
2. Close Youwee completely. If it is minimized to the system tray, exit it from the tray as well.
3. Prefer **Settings → About → Check for updates** for a normal update.
4. If updating manually, download the newer Full Setup and install it over the existing version. You do not need to uninstall the old version first.

Your settings, Library, watchlist, and logs are stored separately from the application files. A normal in-place update is designed to preserve them.

#### If Windows SmartScreen displays a warning

Only continue if the installer came from this repository's release page. Select **More info → Run anyway**, then approve the installer. You can verify the file checksum first using the instructions below.

#### If Youwee does not open or a dependency is missing

1. Restart Windows once after the first installation.
2. Reopen **Settings → Dependencies** and use the refresh button beside the affected tool.
3. If you previously installed an older or partial build, uninstall **Youwee**, install the latest **Full Setup**, and reopen the app. Do not manually copy dependency paths from another computer.
4. Open **Logs** in Youwee and use **Export** when reporting a problem. Remove cookies, tokens, private URLs, and other personal information before sharing the file publicly.

#### Updating Youwee on Windows

Youwee checks the custom release channel, not the upstream release channel. Open **Settings → About → Check for updates**, review the changelog, then select **Update Now**. The app downloads the signed update, installs it, and reopens automatically.

### Install on macOS

The macOS builds are ad-hoc signed, but are not yet signed and notarized with an Apple Developer ID. Download them only from the [official custom-fork releases](https://github.com/anhtahaylove/youwee/releases).

1. **Choose the correct build**
   - Open **Apple menu  → About This Mac**.
   - If it shows **Chip: Apple M1, M2, M3, M4, or newer**, download `Youwee-Mac-Apple-Silicon.dmg`.
   - If it shows **Processor: Intel**, download `Youwee-Mac-Intel.dmg`.
2. **Install Youwee**
   - Open the downloaded `.dmg` file.
   - Drag **Youwee** into the **Applications** folder shown in the installer window.
   - Eject the Youwee disk image, then open **Applications → Youwee**.
3. **If macOS blocks the first launch**
   - Try opening Youwee once so macOS records the blocked launch.
   - Open **System Settings → Privacy & Security**, scroll to **Security**, then click **Open Anyway** next to Youwee.
   - Confirm **Open** when prompted. This exception applies only to this copy of Youwee.
4. **If macOS says Youwee is damaged and cannot be opened**
   - Confirm that the app came from this repository's release page.
   - Open **Terminal**, run the following command, then open Youwee again:

     ```bash
     xattr -dr com.apple.quarantine /Applications/Youwee.app
     ```

For Apple's explanation of these security prompts, see [Safely open apps on your Mac](https://support.apple.com/102445).

### Install on Linux

Current Linux release assets target 64-bit Intel/AMD systems (`x86_64`/`amd64`). ARM Linux builds are not currently published.

Before downloading, you can confirm your architecture with:

```bash
uname -m
```

Continue if it prints `x86_64`. If it prints `aarch64`, `arm64`, or another value, the current Linux release is not a native build for your device.

#### Debian or Ubuntu 22.04 and newer — recommended

1. Download `Youwee-Linux.deb`.
2. Open a terminal and install it with APT. Using APT instead of double-clicking or running `dpkg -i` lets Linux resolve required system packages automatically:

   ```bash
   cd ~/Downloads
   sudo apt install ./Youwee-Linux.deb
   ```

3. Start Youwee from your application menu.
4. Open **Settings → Dependencies** and confirm the detected tools before the first download.

To upgrade later, download the newer `.deb` and run the same command again. Your Youwee settings and Library database remain in your user data directory.

To remove the app later:

```bash
sudo apt remove youwee
```

This removes the application package. Review your user data separately before deleting it manually.

#### Other x86_64 Linux distributions — portable AppImage

1. Download `Youwee-Linux.AppImage`.
2. Make it executable and run it:

   ```bash
   cd ~/Downloads
   chmod +x Youwee-Linux.AppImage
   ./Youwee-Linux.AppImage
   ```

The AppImage is portable: it does not run an installer or automatically add a desktop-menu entry. Keep it in a stable folder such as `~/Applications` if you plan to use it regularly:

```bash
mkdir -p ~/Applications
mv ~/Downloads/Youwee-Linux.AppImage ~/Applications/
chmod +x ~/Applications/Youwee-Linux.AppImage
~/Applications/Youwee-Linux.AppImage
```

If the AppImage reports a FUSE error, use the `.deb` package on Debian/Ubuntu or follow the [official AppImage FUSE troubleshooting guide](https://docs.appimage.org/user-guide/troubleshooting/fuse.html). As a temporary fallback, you can try:

```bash
./Youwee-Linux.AppImage --appimage-extract-and-run
```

After the first launch, check **Settings → Dependencies**. The Windows Full dependency pack is Windows-only, so availability of optional tools can differ on Linux.

#### Linux troubleshooting

- **The `.deb` reports unmet dependencies:** run `sudo apt --fix-broken install`, then repeat `sudo apt install ./Youwee-Linux.deb`.
- **The AppImage reports a permission error:** run `chmod +x Youwee-Linux.AppImage` again and make sure the file is on a Linux filesystem that permits execution.
- **The window does not open:** start Youwee from a terminal and include the terminal output when reporting the issue.
- **Wayland-specific display issue:** log in to an X11/Xorg session for a comparison test before filing a bug.
- **Need help:** open a [custom-fork issue](https://github.com/anhtahaylove/youwee/issues) and include the distribution name, version, desktop environment, output of `uname -m`, and sanitized Youwee logs.

### Verify downloaded files — optional but recommended

Every release includes [`SHA256SUMS.txt`](https://github.com/anhtahaylove/youwee/releases/latest/download/SHA256SUMS.txt). Calculate the SHA-256 checksum for your downloaded file and compare it with the matching line in that file.

**Windows PowerShell**

```powershell
Get-FileHash "$HOME\Downloads\Youwee-Windows-Full-Setup.exe" -Algorithm SHA256
```

**macOS Terminal**

```bash
# Apple Silicon
shasum -a 256 ~/Downloads/Youwee-Mac-Apple-Silicon.dmg

# Intel
shasum -a 256 ~/Downloads/Youwee-Mac-Intel.dmg
```

**Linux terminal**

```bash
sha256sum ~/Downloads/Youwee-Linux.deb
# or
sha256sum ~/Downloads/Youwee-Linux.AppImage
```

PowerShell's `Get-FileHash` uses SHA-256 when requested explicitly; see [Microsoft's Get-FileHash documentation](https://learn.microsoft.com/powershell/module/microsoft.powershell.utility/get-filehash). For AppImage execution details, see the [official AppImage quickstart](https://docs.appimage.org/introduction/quickstart.html).

### Browser Extension (Chromium + Firefox)

| Browser | Download |
|---------|----------|
| **Chromium** (Chrome/Edge/Brave/Opera/Vivaldi/Arc/Coc Coc) | Bundled with the custom Windows installer, or use upstream extension assets |
| **Firefox** | [Install from Mozilla Add-ons](https://addons.mozilla.org/firefox/addon/youwee-download-companion/) |

- One-click send current page to Youwee with `Download now` or `Add to queue`
- Floating button supports `Video/Audio` + quality selection on supported sites
- Popup works on any valid HTTP/HTTPS tab
- Guide: [youwee.app/docs/browser-extension](https://youwee.app/docs/browser-extension)

### Plugins

Extend Youwee with signed `.ywp` plugins for post-download workflows such as notifications, uploads, and third-party integrations.

- Recommended plugins and install guide: [PLUGINS.md](PLUGINS.md)
- SDK: [sdk-js/README.md](sdk-js/README.md) · [youwee-sdk](https://www.npmjs.com/package/youwee-sdk)

### Build from Source

#### Prerequisites

- [Bun](https://bun.sh/) (v1.3.5 or later)
- [Rust](https://www.rust-lang.org/) (v1.70 or later)
- [Tauri CLI](https://tauri.app/v1/guides/getting-started/prerequisites)

#### Steps

```bash
# Clone the repository
git clone https://github.com/anhtahaylove/youwee.git
cd youwee

# Install dependencies
bun install

# Run in development mode
bun run tauri dev

# Build for production
bun run tauri build
```

## Sponsor

<div>
  <a href="https://www.atlascloud.ai/">
    <img src="docs/sponsors/atlascloud.svg" alt="Atlas Cloud" width="220">
  </a>
</div>

## Contributing

This fork is used for custom Windows builds and upstreamable experiments. Neutral fixes are split into clean PR branches against [vanloctech/youwee](https://github.com/vanloctech/youwee) when they are useful beyond this fork. See [Contributing Guide](CONTRIBUTING.md).

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Contact

- **Website**: [youwee.app](https://youwee.app)
- **Discord**: [Youwee Community](https://discord.gg/yCrs9hcw)
- **Custom fork**: [@anhtahaylove/youwee](https://github.com/anhtahaylove/youwee)
- **Upstream**: [@vanloctech/youwee](https://github.com/vanloctech/youwee)
- **Issues**: [Custom fork issues](https://github.com/anhtahaylove/youwee/issues)

---

## Star History

<picture>
  <source
    media="(prefers-color-scheme: dark)"
    srcset="
      https://api.star-history.com/svg?repos=vanloctech/youwee,anhtahaylove/youwee&type=Date&theme=dark
    "
  />
  <source
    media="(prefers-color-scheme: light)"
    srcset="
      https://api.star-history.com/svg?repos=vanloctech/youwee,anhtahaylove/youwee&type=Date
    "
  />
  <img
    alt="Star History Chart"
    src="https://api.star-history.com/svg?repos=vanloctech/youwee,anhtahaylove/youwee&type=Date"
  />
</picture>
