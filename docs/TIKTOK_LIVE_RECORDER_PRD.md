# TikTok Live Recorder for Youwee custom

## Mục tiêu

Thêm tính năng ghi TikTok Live trực tiếp trong Youwee, dùng cấu hình cookie/proxy sẵn có của Youwee. Bản đầu tiên dùng Firefox profile đã chọn trong Settings → Network & Authentication, không dùng ShardBrowser, Docker, Telegram mini-app, hay queue riêng từ repo tham khảo.

## Nguồn tham khảo

- Repo local tham khảo: `C:\Users\Administrator\Documents\GitHub\tiktok-live-recorder`
- Luồng cần học:
  - `src/core/tiktok_api.py`: resolve user/room/live info.
  - `src/core/tiktok_recorder.py`: vòng đời record.
  - `src/http_utils/http_client.py`: browser-like headers/cookie/proxy.
  - `scripts/windows/record-tiktok-elements-stream.ps1`: parse `stream_data`, chọn variant, chạy FFmpeg `-c copy`, ẩn signed URL/cookie.
- Không bê nguyên:
  - ShardBrowser capture.
  - Docker wrapper.
  - Telegram controller/mini-app.

## Phase 1: manual inspect + record

### Backend

1. Nhận input:
   - `@username`
   - `username`
   - `https://www.tiktok.com/@username`
   - `https://www.tiktok.com/@username/live`
   - room id dùng TikTok Webcast room/info API tối thiểu để inspect/record khi có stream_data hoặc legacy stream URL.
2. Reuse network settings hiện có:
   - `cookieMode`
   - `cookieBrowser`
   - `cookieBrowserProfile`
   - `cookieFilePath`
   - `cookieSkipPatterns`
   - `proxyUrl`
3. Inspect live:
   - Dùng helper `run_ytdlp_json_with_cookies` trước vì đã có Firefox profile normalization.
   - Trả metadata đã sanitize: title, uploader, thumbnail, live status, formats/variants.
   - Không trả signed URL ra frontend.
4. Record live:
   - Dùng FFmpeg path từ `get_ffmpeg_path`.
   - Command mục tiêu: copy stream vào MP4/MKV, ưu tiên không transcode.
   - Header/cookie/signed URL chỉ nằm trong process args/temp file nội bộ, không log raw.
   - Khi record bằng FFmpeg, lấy Cookie header từ Firefox profile hoặc Netscape cookie file nếu cấu hình có.
   - Hỗ trợ cancel và xóa file dở.
5. Library/history:
   - Sau khi record xong, gọi `add_history_internal`.
   - `source = "tiktok-live"`.
   - Title fallback: `TikTok LIVE @username`.

### UI

Thêm trang/section nhỏ:

- Input: TikTok username/URL.
- Quality: Auto trước; danh sách variant sau inspect.
- Output folder.
- Optional duration test.
- Buttons: Inspect Live, Start Recording, Cancel.
- Status: live/offline, selected variant, file output, lỗi đã sanitize.

## Phase 2A: resilience + auto-reconnect

Đã triển khai trong custom worktree:

- Retry metadata tối đa 3 lần với backoff ngắn cho timeout, lỗi mạng/process và JSON tạm lỗi.
- Không retry trạng thái TikTok Live đang offline hoặc lỗi không retryable.
- FFmpeg auto-reconnect mặc định bật, giới hạn 20 lần và tổng thời gian chờ 120 giây.
- Giữ file ghi được và ghi Library/history dạng partial nếu reconnect hết hạn sau khi đã có dữ liệu.
- Cancel hoạt động cả khi đang chuẩn bị metadata, không chỉ sau khi FFmpeg đã chạy.
- UI phân biệt Preparing, retry metadata, Recording, Cancelling và partial recording.
- Lỗi backend wire được unwrap/sanitize, không hiện hoặc lưu raw `__YOUWEE_ERR__` trong log TikTok Live.

## Phase 2B (deferred)

- Watchlist/polling chờ streamer online.
- Schedule auto-record.
- Telegram Remote Download command cho TikTok Live.
- Mở rộng native TikTok API/page resolver nếu cần username → room_id không phụ thuộc yt-dlp.

## Không làm trong Phase 1

- Không dùng ShardBrowser.
- Không thêm dependency mới.
- Không viết queue FSM riêng.
- Không lưu signed stream URL/cookie vào DB/log/UI.
- Không đụng luồng YouTube/Facebook/Instagram hiện có.

## Test spec

### Unit tests

- Parse input TikTok:
  - `@abc`
  - `abc`
  - `https://www.tiktok.com/@abc` → `https://www.tiktok.com/@abc/live`
  - `https://www.tiktok.com/@abc/live`
  - mobile URL nếu thêm resolver.
- Format/variant:
  - Không expose `url`.
  - Auto chọn variant điểm cao nhất theo resolution/bitrate.
  - Filter transport/quality hoạt động.
- Firefox profile:
  - Reuse test hiện có cho `i879pxds.default-release`.
  - Cookie header nội bộ đọc được từ Firefox profile/cookie file cho FFmpeg khi cần.
- Secret redaction:
  - Log/result không chứa signed URL/cookie.

### Manual acceptance

1. Login TikTok trong Firefox profile `i879pxds.default-release`.
2. Settings → Network:
   - Cookie Source: From Browser
   - Browser: Firefox
   - Profile: `i879pxds.default-release (default-release)`
3. Inspect một TikTok Live thật.
4. Record 30–60 giây.
5. Mở file MP4/MKV.
6. Library có bản ghi:
   - source `tiktok-live`
   - title đọc được
   - filepath đúng
7. Cancel record:
   - status là cancelled
   - không để file dở.
8. Logs không có cookie hoặc signed stream URL.

## Required checks

- `bun run biome check --write .`
- `bun run tsc -b`
- `cargo check` trong `src-tauri`
- Targeted Rust tests cho TikTok Live helper
- Full NSIS build chỉ chạy khi cần đóng gói.

