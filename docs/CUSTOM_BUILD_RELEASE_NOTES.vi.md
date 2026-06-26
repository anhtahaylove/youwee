# Youwee Custom Build - Facebook Reel Core Fallback

Ngày build: 2026-06-26

## Điểm chính

- Thêm fallback trong core `download_video` cho Facebook Reels public khi yt-dlp báo `[facebook] Cannot parse data`.
- Fallback chỉ áp dụng cho URL `facebook.com/reel/...` hoặc subdomain Facebook tương đương; các site và path Facebook khác không bị đổi.
- Khi fallback chạy, core bỏ browser cookies và Aria2, dùng `--downloader native`, `--impersonate chrome`, output MP4 ngắn theo dạng `facebook-com-reel-ID.mp4`.
- Core probe JSON metadata không dùng cookie trước khi retry, rồi ghi title và thumbnail vào queue, Library/history.
- Fallback thành công đi qua flow download chuẩn của Youwee, nên Library, progress, retry và cancel vẫn thuộc core app.
- Plugin `local.without-cookie-fallback` có thể còn được cài trong app data cũ, nhưng core fallback không phụ thuộc plugin này.

## Đã kiểm thử live

Các URL sau đều đi qua lỗi primary `Cannot parse data`, được core fallback retry thành công, và có title/thumbnail trong Library:

- `https://www.facebook.com/reel/1137385980554926`
- `https://www.facebook.com/reel/1302595173728654`
- `https://www.facebook.com/reel/1159891442771840`
- `https://www.facebook.com/reel/1889836315019111`

Ảnh kiểm tra UI không nằm trong workspace GitHub hiện tại; nếu cần đối chiếu lại UI, chụp lại artifact mới trong `C:\Users\Administrator\Documents\GitHub\youwee`.

## Migration từ plugin cũ

- Nếu `local.without-cookie-fallback` vẫn còn trong **Settings -> Plugins -> Workflows**, có thể tắt hoặc gỡ khỏi trigger `download.queued` để kiểm thử core fallback độc lập.
- Không cần cài lại plugin này cho Facebook Reels public; core download path đã tự xử lý retry không cookie và ghi Library/history.
- Nếu giữ plugin vì workflow khác, fallback trong core vẫn chạy độc lập khi yt-dlp báo `[facebook] Cannot parse data`.

## Giới hạn

- Không xử lý video private hoặc video cần login thật sự.
- Không fallback cho lỗi khác ngoài `[facebook] Cannot parse data`.
- Nếu Facebook hoặc yt-dlp đổi extractor, cần test lại classifier lỗi trước khi mở rộng.
