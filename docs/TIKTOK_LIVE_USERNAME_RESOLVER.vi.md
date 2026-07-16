# Thiết kế fallback username resolver cho TikTok Live

## Mục tiêu

Phân giải `@username` hoặc URL TikTok Live thành `room_id` mà không để một dịch vụ ký URL bên thứ ba trở thành điểm lỗi duy nhất. Luồng phải tiếp tục hỗ trợ Firefox profile, watchlist chạy nền và input `room_id` trực tiếp.

## Nguyên tắc

- Ưu tiên dữ liệu TikTok first-party và thông tin đã có trên máy người dùng.
- Không gửi cookie TikTok, signed stream URL hoặc dữ liệu Firefox profile cho dịch vụ bên thứ ba.
- Chỉ kết luận `offline` khi dữ liệu first-party xác nhận; lỗi resolver phải trả về `resolver-unavailable` thay vì giả thành offline.
- Cache chỉ lưu username chuẩn hóa, `room_id`, thời điểm quan sát và session marker; không lưu cookie hoặc stream URL có chữ ký.
- Không thêm browser automation/headless Firefox trong giai đoạn đầu vì nặng và dễ vỡ.

## Chuỗi phân giải đề xuất

1. **Room ID trực tiếp**
   - Nếu input chỉ gồm chữ số, dùng ngay Webcast `room/info` hiện có.
   - Đây là đường dẫn ổn định nhất và không cần dịch vụ ngoài.

2. **Cache cục bộ có TTL**
   - Tra mapping `username -> room_id` của lần inspect thành công gần nhất.
   - Xác minh lại bằng `room/info` trước khi dùng; loại cache nếu owner/session không khớp hoặc live đã kết thúc.
   - TTL đề xuất: 10 phút khi online, 60 giây khi chưa xác định được trạng thái.

3. **yt-dlp với authentication hiện có**
   - Dùng cookie mode, Firefox profile, proxy và binary path đã cấu hình trong Youwee.
   - Nếu yt-dlp trả được `room_id` hoặc metadata live thì cập nhật cache.

4. **TikTok page hydration first-party**
   - Tải URL `https://www.tiktok.com/@username/live` với browser-like headers và cookie Firefox nếu có.
   - Tái sử dụng parser hiện có cho `SIGI_STATE` và `__UNIVERSAL_DATA_FOR_REHYDRATION__` để lấy `room_id`, trạng thái live và metadata.
   - Phân loại riêng HTTP `403`, `429` và `5xx` để watchlist áp dụng backoff phù hợp.

5. **Browser extension bridge (giai đoạn 2)**
   - Khi người dùng đang mở tab TikTok, extension đọc page hydration rồi gửi duy nhất `{ username, room_id, observed_at }` về app qua cơ chế deep-link hiện có.
   - Đây là nguồn cơ hội cho inspect thủ công; watchlist chạy nền không được phụ thuộc vào việc Firefox đang mở.

6. **Remote resolver tùy chọn, dùng cuối cùng**
   - TikRec hoặc provider tương lai chỉ là fallback sau cùng.
   - Áp dụng timeout ngắn, circuit breaker và cooldown theo provider; một provider hỏng không chặn provider khác hay first-party flow.
   - Không tạo abstraction/plugin framework cho đến khi có ít nhất hai provider thực sự hoạt động.

## Trạng thái và lỗi

- `online`: `room/info` hoặc page hydration xác nhận phiên live đang hoạt động.
- `offline`: nguồn first-party trả trạng thái live đã kết thúc/chưa bắt đầu.
- `resolver-unavailable`: hết chuỗi nhưng không có nguồn nào xác nhận được online/offline.
- `rate-limited`: gặp `429`; watchlist tăng backoff có jitter.
- `authentication-required`: nội dung yêu cầu cookie/profile hợp lệ.

## Lộ trình triển khai nhỏ

### Phase 1 — Không phụ thuộc dịch vụ duy nhất

- Thêm cache SQLite cho mapping đã xác minh.
- Đổi thứ tự sang room ID -> cache xác minh -> yt-dlp -> page hydration -> remote resolver.
- Chuẩn hóa lỗi và thêm test cho stale cache, 403/429/5xx.

### Phase 2 — Extension bridge

- Bổ sung message/deep-link chỉ chứa username, room ID và timestamp.
- Xác minh room ID ở Rust backend trước khi ghi cache.

### Phase 3 — Remote health (chỉ khi cần)

- Theo dõi lỗi liên tiếp, timeout và cooldown cho từng provider.
- Chỉ trừu tượng hóa provider khi có provider thứ hai đã được kiểm chứng.

## Tiêu chí nghiệm thu

- Username online vẫn inspect được khi TikRec bị tắt hoặc trả `5xx`.
- Room ID trực tiếp luôn bỏ qua username resolver.
- Cache cũ không được gắn nhầm sang phiên live mới.
- Watchlist không ghi `offline` khi thực tế chỉ là lỗi mạng/resolver.
- Logs không chứa cookie, signed stream URL hoặc đường dẫn nhạy cảm của Firefox profile.
- Test xác nhận `403`, `429`, timeout và provider `5xx` có trạng thái/backoff đúng.
