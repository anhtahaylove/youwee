# Zero-Setup Dependency Pack cho Youwee

## Mục tiêu

Người dùng Windows cài Youwee một lần rồi có thể tải ngay, không cần tự cài
`yt-dlp`, FFmpeg, Deno hoặc `gallery-dl`.

## Quyết định

- Làm **Windows-first Full Installer**.
- Giữ `yt-dlp` ở kênh **Bundled** làm mặc định.
- Đóng gói `ffmpeg.exe`, `ffprobe.exe`, `deno.exe` và `gallery-dl.exe` dưới dạng
  Tauri resources; không chạy trình cài đặt hoặc script bên ngoài khi app mở lần
  đầu.
- Giữ cơ chế app-managed hiện tại làm lớp cập nhật: binary trong app data được
  ưu tiên, resource đi kèm app là fallback luôn sẵn có, system binary là lựa chọn
  nâng cao.
- `YouTube Troubleshooting` giữ mặc định **Auto** vì đây là preset cấu hình,
  không phải dependency.

## Thứ tự tìm binary

1. Binary app-managed trong `AppData/Roaming/com.vanloctech.youwee/bin`.
2. Binary đóng gói trong resource directory của Youwee.
3. Binary hệ thống nếu người dùng chọn hoặc chế độ `Auto` cần fallback.

Không sao chép resource sang app data ở lần chạy đầu. Chạy trực tiếp resource
giúp tránh nhân đôi hàng trăm MB; bản cập nhật tải trong app data vẫn có thể ghi
đè mà không khóa file của installer.

## Manifest build tái lập được

Thêm một manifest nhỏ, ví dụ `src-tauri/dependencies.lock.json`, chứa cho từng
platform:

- tên dependency và phiên bản đã kiểm thử;
- URL artifact cố định;
- SHA-256;
- tên binary sau khi giải nén;
- giấy phép và URL source.

Workflow release tải artifact theo manifest, kiểm tra SHA-256, chạy `--version`
và chỉ sau đó mới build installer. Không dùng URL `latest` mà không có checksum
được pin; checksum sai phải làm build fail.

Các binary lớn không commit vào Git. Nếu upstream không cung cấp URL bất biến,
mirror artifact đã kiểm tra vào một release dependency riêng ngay trong
`anhtahaylove/youwee`, để vẫn chỉ duy trì một repository.

## Dependency Windows đề xuất

| Dependency | Vai trò | Nguồn build |
| --- | --- | --- |
| yt-dlp | Extractor/download core | Official stable release, đã có sidecar |
| FFmpeg + ffprobe | Merge, remux, metadata, thumbnail, media tools | Bản x64 đã pin checksum |
| Deno | JavaScript runtime cho extractor YouTube | Official Deno x86_64 Windows ZIP |
| gallery-dl | Gallery, collection và creator feed | Standalone x64 từ build được upstream giới thiệu, pin checksum |

Installer Full dự kiến tăng khoảng 200–260 MB. Giữ thêm installer nhẹ là tùy
chọn phát hành, nhưng asset được khuyên dùng cho tester/người không chuyên phải
là `Youwee-Windows-Full-Setup.exe`.

## Thay đổi tối thiểu trong source

1. Thêm helper tìm resource dependency và dùng lại trong `ffmpeg.rs`, `deno.rs`
   và `gallerydl.rs`.
2. Sửa thứ tự resolver theo mục trên; không tạo dependency manager mới.
3. Thêm resources vào cấu hình Tauri Windows release.
4. Thêm bước prepare/verify dependencies vào `.github/workflows/build.yml`.
5. Settings hiển thị `Included with Youwee` khi đang dùng resource; nút tải/cập
   nhật hiện tại vẫn cài bản mới vào app data.
6. Gallery không còn bắt người dùng tự cài system `gallery-dl` khi resource tồn
   tại.

## Bảo mật và giấy phép

- Kiểm tra SHA-256 trước build và trước khi chấp nhận runtime update.
- Ghi version/path đang dùng vào Logs để hỗ trợ chẩn đoán.
- Đóng gói `THIRD_PARTY_NOTICES` cùng giấy phép và source URL của FFmpeg,
  gallery-dl, Deno và yt-dlp.
- Không tải hoặc thực thi binary không có trong manifest.

## Acceptance test

Trên Windows VM sạch, tắt mạng sau khi cài:

1. Settings > Dependencies báo cả bốn dependency sẵn sàng và có path trong app.
2. Tải YouTube cần merge video/audio thành công bằng FFmpeg.
3. Extractor cần JavaScript runtime chạy bằng Deno.
4. Tải một gallery bằng `gallery-dl` mà không cần Python, pip, Chocolatey hoặc
   Scoop.
5. Gỡ các system binary khỏi `PATH` vẫn không làm ba test trên thất bại.
6. Cập nhật một dependency trong app data, restart và xác nhận app dùng bản mới;
   xóa bản app-managed thì tự quay về resource.
7. Updater nâng app mà không xóa binary app-managed hoặc cấu hình người dùng.

## Phạm vi triển khai đề xuất

- **Phase 1:** Windows Full Installer, resolver/resource priority và CI checksum.
- **Phase 2:** Settings polish, offline VM acceptance và release asset Full.
- **Phase 3:** macOS/Linux sau khi có artifact ổn định, ký/notarize và executable
  permission tương ứng.
