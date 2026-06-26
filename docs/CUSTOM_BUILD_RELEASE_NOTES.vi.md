# Youwee Custom Build - Facebook Reel Core Fallback

Ngày build: 2026-06-26

## Điểm chính

- Thêm fallback trong core `download_video` cho Facebook Reels public khi yt-dlp báo `[facebook] Cannot parse data`.
- Fallback chỉ áp dụng cho URL `facebook.com/reel/...` hoặc subdomain Facebook tương đương; các site và path Facebook khác không bị đổi.
- Khi fallback chạy, core bỏ browser cookies và Aria2, dùng `--downloader native`, `--impersonate chrome`, output MP4 ngắn theo dạng `facebook-com-reel-ID.mp4`.
- Core probe JSON metadata không dùng cookie trước khi retry, rồi ghi title và thumbnail vào queue, Library/history.
- Fallback thành công đi qua flow download chuẩn của Youwee, nên Library, progress, retry và cancel vẫn thuộc core app.

## Đã kiểm thử live

Các URL sau đều đi qua lỗi primary `Cannot parse data`, được core fallback retry thành công, và có title/thumbnail trong Library:

- `https://www.facebook.com/reel/1137385980554926`
- `https://www.facebook.com/reel/1302595173728654`
- `https://www.facebook.com/reel/1159891442771840`
- `https://www.facebook.com/reel/1889836315019111`

Ảnh kiểm tra UI:

- Queue: `C:\Users\Administrator\Documents\Codex\2026-06-25\c\youwee-core-fallback-queue.png`
- Library: `C:\Users\Administrator\Documents\Codex\2026-06-25\c\youwee-core-fallback-library.png`

## Giới hạn

- Không xử lý video private hoặc video cần login thật sự.
- Không fallback cho lỗi khác ngoài `[facebook] Cannot parse data`.
- Nếu Facebook hoặc yt-dlp đổi extractor, cần test lại classifier lỗi trước khi mở rộng.
