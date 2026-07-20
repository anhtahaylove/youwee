# Nhật ký thay đổi

Tất cả thay đổi đáng chú ý của Youwee sẽ được ghi lại trong file này.

Định dạng dựa trên [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
và dự án tuân theo [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.19.1-custom.44] - 2026-07-20

### Sửa lỗi
- **Khôi phục Queue sau khi xóa trong Library** - Đối chiếu lại item Universal và YouTube đã lưu khi file tải xuống cùng bản ghi lịch sử bị xóa, đưa item về Pending để có thể tải lại mà không cần thêm URL lần nữa

## [0.19.1-custom.43] - 2026-07-20

### Sửa lỗi
- **Tên file Facebook Reel nhất quán** - Dùng lại tiêu đề metadata đã khôi phục cho cả placeholder `Video` khi có cookie và tiêu đề có tiền tố tương tác khi không dùng cookie, giúp Queue, Library và file tải về đồng bộ

## [0.19.1-custom.42] - 2026-07-19

### Thay đổi
- **Phản hồi CI nhanh hơn** - Hủy lượt CI cũ trên cùng branch hoặc pull request và dùng checkout v6 cho các workflow chạy trên GitHub-hosted runner
- **Phát hành custom an toàn hơn** - Thêm helper bump version mặc định dry-run, kiểm tra ba version ứng dụng đồng bộ, chuyển Unreleased của cả ba changelog, suy ra version Windows Installer và giữ version extension store độc lập

## [0.19.1-custom.41] - 2026-07-18

### Thêm mới
- **Khuyến nghị cookie được quản lý** - Làm mới danh mục bỏ qua cookie toàn cục đã xác thực với cache 24 giờ, đồng thời giữ quy tắc cá nhân ở máy người dùng và cấu hình độc lập

### Thay đổi
- **Thay thế dependency an toàn hơn** - Cập nhật FFmpeg và ffprobe thành một cặp đã xác minh, có thể rollback, đồng thời kiểm tra binary Deno tải về trước khi kích hoạt
- **Mặc định cookie thận trọng** - Giữ danh mục khuyến nghị toàn cục trống cho đến khi một website thực sự ổn định hơn khi bỏ qua xác thực, nhưng vẫn bảo toàn quy tắc cá nhân và migration cũ

### Sửa lỗi
- **Tên file Facebook Reel trên Windows** - Giới hạn title template theo thư mục output thực tế để các file video/audio trung gian có tiêu đề dài hoặc Unicode vẫn ghi được
- **Trạng thái dependency đồng nhất** - Hiển thị Đã mới nhất cho FFmpeg, Deno và gallery-dl sau khi kiểm tra thành công thay vì quay về nhãn được đóng gói cùng Youwee

## [0.19.1-custom.39] - 2026-07-18

### Thêm mới
- **Cập nhật gallery-dl do ứng dụng quản lý** - Kiểm tra kênh phát hành gallery-dl chính thức khi chạy và cho phép người dùng cập nhật binary do ứng dụng quản lý sau khi xác nhận
- **Kênh yt-dlp rõ ràng** - Hiển thị phiên bản đã cài và khả dụng cho Bundled, Stable, Nightly và kênh Master thử nghiệm

### Thay đổi
- **Đơn giản hóa cài đặt cookie** - Xóa các chip Quick presets nhưng vẫn giữ quy tắc bỏ qua cookie mặc định cho Facebook Reel và cấu hình rõ ràng của người dùng

### Sửa lỗi
- **Trạng thái gallery-dl đã mới nhất** - Chấp nhận mã thoát khác 0 của gallery-dl khi nội dung trả về xác nhận binary do ứng dụng quản lý đã là bản mới nhất

## [0.19.1-custom.38] - 2026-07-18

### Thêm mới
- **Cài extension Chromium thân thiện cho người mới** - Kèm thư mục extension Chromium đã giải nén trong Full Installer Windows, thêm nút mở thư mục, sao chép thông tin cài đặt và hướng dẫn offline

## [0.19.1-custom.37] - 2026-07-17

### Thay đổi
- **Tách version extension** - Đọc version tiện ích từ manifest để release ứng dụng desktop không còn xung đột số version trên AMO

### Sửa lỗi
- **Tương thích Gatekeeper trên macOS** - Ký ad-hoc và xác minh app bundle macOS để người dùng Apple Silicon có thể mở build thủ công qua Quyền riêng tư & Bảo mật trước khi có Developer ID notarization

## [0.19.1-custom.36] - 2026-07-17

### Thêm mới
- **Thông tin truyền tải updater** - Hiển thị tốc độ tải hiện tại, thời gian còn lại ước tính và giai đoạn đang cài đặt khi cập nhật ứng dụng
- **Bộ phát hành extension công khai** - Thêm package AMO listed, nội dung trang sản phẩm đã bản địa hóa, tài liệu quyền riêng tư và ảnh store sẵn dùng cho Firefox/Chrome

### Thay đổi
- **Quyền trình duyệt tối thiểu** - Xóa quyền tabs không sử dụng và thay phạm vi web-accessible rộng bằng allowlist các website media được hỗ trợ

## [0.19.1-custom.35] - 2026-07-17

### Sửa lỗi
- **Fallback changelog sau cập nhật** - Lấy metadata release qua Rust backend để bản nâng cấp từ build cũ vẫn hiển thị đầy đủ ghi chú khi WebView bị CORS chặn request tới asset GitHub

## [0.19.1-custom.34] - 2026-07-17

### Thêm mới
- **Nhật ký sau cập nhật** - Hiển thị changelog bản địa hóa một lần sau khi cập nhật trong app thành công và mở lại Youwee

### Thay đổi
- **Cập nhật Windows yên lặng** - Cài bản cập nhật theo tài khoản người dùng mà không mở lại giao diện installer tương tác

### Sửa lỗi
- **Dọn bộ nhớ tạm updater** - Xóa các gói cập nhật Youwee cũ khỏi thư mục tạm của hệ thống sau khi app khởi động

## [0.19.1-custom.33] - 2026-07-17

### Thêm mới
- **Preset bỏ qua cookie cho nội dung public** - Thêm chip YouTube và Instagram một chạm để bổ sung pattern URL public cụ thể mà không thay thế quy tắc xác thực hiện có

## [0.19.1-custom.32] - 2026-07-17

### Thay đổi
- **Hướng dẫn cookie Firefox** - Làm rõ Firefox có thể tiếp tục mở khi Youwee đọc cookie từ profile đã chọn
- **Đồng bộ xác thực Gallery** - Dùng chung cơ chế chuẩn hóa browser profile và bỏ qua cookie theo URL cho tải Gallery

### Sửa lỗi
- **Khôi phục file Universal Queue** - Chuyển item Completed về Pending khi file media đã bị xóa để có thể tải lại trực tiếp mà không cần Add URL lần nữa
- **Điều hướng lỗi Queue** - Cho nút Logs trên item thất bại ở YouTube, Universal và Gallery mở đúng trang Logs để xử lý sự cố

## [0.19.1-custom.31] - 2026-07-17

### Thêm mới
- **Tự cập nhật tiện ích Firefox** - Cho phép tiện ích Firefox tự phân phối có chữ ký tìm và cài XPI mới hơn qua update manifest GitHub được xác minh SHA-256

### Thay đổi
- **Toàn vẹn release Firefox** - Kiểm định tiện ích theo chế độ self-hosted và yêu cầu signed XPI cùng `firefox-updates.json` trước khi phát hành release

## [0.19.1-custom.30] - 2026-07-16

### Thay đổi
- **Consent dữ liệu Firefox** - Khai báo thao tác chuyển URL trang hiện tại là dữ liệu hoạt động duyệt web bắt buộc và yêu cầu phiên bản Firefox hỗ trợ consent tích hợp của Mozilla

### Sửa lỗi
- **Kiểm định Firefox AMO** - Thay các lệnh ghi `innerHTML` không an toàn bằng markup tĩnh và DOM API gốc để gói tiện ích được kiểm định không còn cảnh báo

## [0.19.1-custom.29] - 2026-07-16

### Thêm mới
- **Windows Full Installer** - Đóng gói sẵn yt-dlp, FFmpeg/ffprobe, Deno và gallery-dl đã khóa checksum để máy Windows mới có thể tải ngay mà không cần cài dependency thủ công

### Thay đổi
- **Nhận diện dependency linh động** - Phân giải binary app-managed, packaged và system từ thư mục runtime của từng máy, đồng thời hiển thị rõ công cụ bundled trong Settings
- **Artifact release Windows** - Phát hành Full NSIS và MSI có chữ ký cùng updater metadata tương ứng và thông báo bản quyền bên thứ ba

### Sửa lỗi
- **Path dependency giữa các máy** - Tự dùng binary Windows packaged khi thiếu công cụ app-managed thay vì phụ thuộc path hoặc dependency của máy build

## [0.19.1-custom.28] - 2026-07-16

### Thêm mới
- **Phân giải username TikTok Live first-party** - Lấy room ID từ trang TikTok trước các fallback bên ngoài và cache ánh xạ username-room vừa xác minh mà không lưu signed stream URL

### Thay đổi
- **Preview metadata Universal** - Dùng probe yt-dlp UTF-8 gọn nhẹ, giữ nguồn thumbnail từ xa và tải lại metadata còn thiếu mà không chặn download
- **Artifact release** - Chuẩn bị đóng gói Windows MSI và Firefox XPI trong workflow release, đồng thời bật ký AMO khi đã cấu hình thông tin Mozilla

### Sửa lỗi
- **Facebook Reel ổn định giữa các máy** - Chuẩn hóa link Reel bằng cách bỏ tham số tracking để metadata, kiểm tra trùng, download và history dùng cùng một URL ổn định
- **Khôi phục dependency yt-dlp** - Ghi rõ binary được chọn cùng lỗi thực thi, rồi tự thử binary bundled khi executable app-managed đã cập nhật không thể khởi động
- **Phản hồi URL trùng trong Universal** - Nhận diện item đã có giữa URL sạch và URL tracking, focus item, thông báo người dùng và tải lại preview còn thiếu thay vì âm thầm thêm trùng

## [0.19.1-custom.27] - 2026-07-16

### Thêm mới
- **Telemetry ghi TikTok Live** - Hiển thị chính xác thời gian đã ghi, tổng thời lượng, thời gian còn lại, sức chứa phòng đang hoạt động và thao tác sao chép/mở URL xuyên suốt Inspect, Record, Watchlist và kết quả hoàn tất

### Thay đổi
- **Preview TikTok Live bền vững** - Ưu tiên ảnh bìa động từ TikTok và chụp snapshot FFmpeg có xác thực, giới hạn thời gian khi không có ảnh bìa đáng tin cậy
- **Điều khiển thời lượng TikTok Live** - Thay ô nhập thời lượng thô bằng các trường giờ, phút và giây rõ ràng, dùng chung helper chuyển đổi đã có regression test

### Sửa lỗi
- **Thumbnail Library TikTok Live** - Tạo thumbnail bền vững từ media đã hoàn tất hoặc khôi phục và resolve ảnh cache local nhất quán trong thẻ Library cùng hộp thoại tóm tắt
- **Lookup username TikTok Live** - Retry lỗi HTTP tạm thời của dịch vụ lookup phòng đã ký và gợi ý room ID dạng số thay vì báo nhầm stream đang live là offline

## [0.19.1-custom.26] - 2026-07-16

### Thêm mới
- **Metadata preview TikTok Live** - Khôi phục tiêu đề, chủ kênh, ảnh bìa và avatar từ metadata phòng/trang TikTok, cache ảnh từ xa qua Youwee và giữ preview mới nhất trong Watchlist
- **Ảnh chụp lượng người xem TikTok Live** - Hiển thị avatar streamer cùng số người đang xem hoặc số gần nhất đã thấy trong Inspect và Watchlist mà không tăng tần suất polling

## [0.19.1-custom.25] - 2026-07-15

### Sửa lỗi
- **Hướng dẫn khởi động lại updater Windows** - Giải thích ngay khi tải rằng trình cài đặt Windows sẽ tự đóng và mở lại Youwee, thay vì khiến người dùng chờ nút Khởi động lại có thể không xuất hiện

## [0.19.1-custom.24] - 2026-07-15

### Thay đổi
- **Ghi nhanh TikTok Live** - Đưa mục tiêu Inspect, preset Bản gốc/60 FPS/chỉ âm thanh, thời lượng thân thiện, thư mục đầu ra và hành động Ghi chính vào luồng màn hình đầu rõ ràng hơn, đồng thời vẫn giữ các tùy chọn nâng cao

### Sửa lỗi
- **Rule tùy chỉnh TikTok Live** - Giữ thời lượng, chu kỳ polling, cooldown và tên file tùy chỉnh luôn sửa được thay vì tự nhảy về preset có giá trị trùng khớp
- **Khôi phục phòng TikTok Live đang hoạt động** - Khôi phục username đang live qua metadata phòng đã ký khi yt-dlp báo nhầm là offline, không lộ stream URL và không gửi cookie trình duyệt tới dịch vụ ký
- **Phóng to header Windows** - Đồng bộ thao tác double-click nhanh trên thanh tiêu đề với trạng thái maximize/restore native, không tự quay lại window mode ngay lập tức

## [0.19.1-custom.23] - 2026-07-15

### Thay đổi
- **Không gian TikTok Live Recorder** - Tách luồng ghi thủ công và tự động hóa watchlist, cải thiện preview stream và phản hồi bản ghi, đồng thời hoàn thiện telemetry, bố cục responsive, trạng thái trống và accessibility

## [0.19.1-custom.22] - 2026-07-14

### Sửa lỗi
- **Logs watchlist TikTok Live** - Không ghi các lượt polling offline dự kiến của watchlist vào error logs, đồng thời vẫn giữ lỗi Inspect thủ công và lỗi metadata thật

## [0.19.1-custom.21] - 2026-07-14

### Sửa lỗi
- **Metadata TikTok Live 60 FPS** - Gọi metadata phòng TikTok với device type web H.265 để chất lượng Bản gốc có thể expose stream `uhd_60` 1080p60 khi TikTok cung cấp

## [0.19.1-custom.20] - 2026-07-13

### Thêm mới
- **Metadata stream TikTok Live** - Hiển thị FPS, codec, transport, độ phân giải và bitrate rõ hơn trong kết quả Inspect và lịch sử Library

### Sửa lỗi
- **Chọn transport TikTok Live** - Ưu tiên stream FLV ổn định hơn HLS khi chất lượng ngang nhau và chỉ hiển thị FPS khi metadata cung cấp rõ ràng

## [0.19.1-custom.19] - 2026-07-13

### Sửa lỗi
- **Chất lượng gốc TikTok Live 60 FPS** - Ưu tiên stream `uhd_60` khi chất lượng Bản gốc có cùng độ phân giải để bản ghi live giữ 60 FPS khi TikTok cung cấp

## [0.19.1-custom.18] - 2026-07-12

### Sửa lỗi
- **Chất lượng gốc TikTok Live** - Fetch metadata trang TikTok Live bằng cookie Firefox và merge stream HEVC `origin`/`uhd_60` để bản ghi dùng được chất lượng nguồn 1080x1920 khi TikTok cung cấp
- **Chọn stream TikTok Live** - Ưu tiên HLS ổn định hơn LLS khi chất lượng và độ phân giải ngang nhau

## [0.19.1-custom.17] - 2026-07-11

### Thêm mới
- **Lệnh Telegram backend cho TikTok Live** - Xử lý các lệnh watchlist `/tl_*` trong backend Rust để điều khiển qua Telegram Topic vẫn hoạt động khi app bị ẩn hoặc minimized

### Sửa lỗi

## [0.19.1-custom.16] - 2026-07-11

### Thêm mới
- **TikTok Live Recorder Phase 3C-6** - Lưu giới hạn recorder qua lần khởi động lại, thêm khung giờ ghi riêng từng streamer, hiển thị tổng telemetry, cảnh báo giới hạn multi-room và xác nhận lệnh watchlist qua Telegram Topic trước release tester đã ký

### Sửa lỗi

## [0.19.1-custom.15] - 2026-07-11

### Thêm mới
- **TikTok Live Recorder Phase 2A** - Thêm tự động kết nối lại FFmpeg có giới hạn, retry metadata với backoff, giữ bản ghi partial và trạng thái vòng đời ghi chi tiết
- **TikTok Live Recorder Phase 2B** - Làm mới signed stream URL hết hạn, ghi tiếp vào segment đánh số, ghép segment không transcode và giữ các phần đã ghi nếu ghép lỗi
- **TikTok Live Recorder Phase 2C** - Ghi segment Matroska an toàn khi crash, remux sang MP4 không transcode và giữ file MKV phát được nếu finalize thất bại
- **TikTok Live Recorder Phase 2D** - Lưu metadata job an toàn trong SQLite, đối chiếu phiên bị gián đoạn khi khởi động và cung cấp Ghi tiếp, Hoàn tất, Xóa có xác nhận mà không lưu signed URL hoặc giá trị cookie
- **TikTok Live Recorder Phase 3A** - Thêm watchlist streamer lưu bền vững với polling backoff có giới hạn, rule ghi riêng từng streamer, tự ghi khi chuyển từ offline sang live, chống ghi trùng toàn cục và đối chiếu an toàn sau khi khởi động lại

### Sửa lỗi
- **Xóa watchlist TikTok Live** - Ngăn polling metadata nền ghi lại streamer đã bị xóa từ Telegram hoặc UI
- **Lỗi và hủy TikTok Live** - Gỡ wire error backend bị lồng, báo stream offline rõ ràng và cho hủy ngay khi metadata còn đang chuẩn bị

## [0.19.1-custom.14] - 2026-07-10

### Thêm mới
- **TikTok Live Recorder Phase 1** - Thêm trang ghi TikTok Live với Inspect, Record, Cancel, xác thực cookie Firefox, và ghi Library/history cho bản ghi hoàn tất

### Sửa lỗi
- **Chất lượng tự động TikTok Live** - Ưu tiên stream live muxed video+audio tốt nhất trước khi fallback sang định dạng chỉ video hoặc chỉ âm thanh

## [0.19.1-custom.13] - 2026-07-09

### Thay đổi
- **Kênh release custom** - Chuyển metadata updater custom, link tải installer và link tải extension từ `anhtahaylove/youwee-releases` sang trang release chính của fork `anhtahaylove/youwee`

## [0.19.1-custom.12] - 2026-07-09

### Sửa lỗi
- **Icon AppImage** - Cập nhật Tauri build tooling và các dependency tương thích để gói AppImage Linux giữ đúng metadata icon desktop

## [0.19.1-custom.11] - 2026-07-08

### Thêm mới
- **Giao diện tiếng Nhật và tiếng Tây Ban Nha** - Thêm bộ dịch tiếng Nhật và tiếng Tây Ban Nha, đồng thời giữ fallback tiếng Anh cho các chuỗi chỉ có trong bản custom

## [0.19.1-custom.10] - 2026-07-08

### Thêm mới
- **AI Summary cho video dài** - Chia transcript dài thành các phần tóm tắt có thể hủy, cho chọn xuất theo từng phần hoặc tóm tắt cuối, và tùy chỉnh số từ mỗi phần

### Sửa lỗi
- **Danh sách đánh số trong AI Summary** - Giữ đúng thứ tự danh sách đánh số qua các block markdown tách rời trong summary đã tạo

## [0.19.1-custom.9] - 2026-07-07

### Thêm mới
- **Codec video cho Universal** - Cho Universal Download chọn H.264, VP9, AV1 hoặc Auto và giữ codec đã chọn trên từng item trong hàng đợi

### Sửa lỗi
- **Tham số summary OpenAI** - Tự động đổi request summary OpenAI và proxy giữa `max_tokens` và `max_completion_tokens` khi model từ chối một dạng tham số

## [0.19.1-custom.8] - 2026-07-07

### Thêm mới
- **Thông báo cập nhật yt-dlp** - Báo cho người dùng khi kênh yt-dlp đang chọn có bản mới và mở luồng cập nhật trong Settings > Dependencies từ toast

### Thay đổi
- **Cỡ chữ khi đọc summary** - Cho summary vừa tạo và summary đã lưu dùng chung điều khiển cỡ chữ trực tiếp để đọc dễ hơn

## [0.19.1-custom.7] - 2026-07-06

### Sửa lỗi
- **Thư mục tải xuống Windows quá dài** - Giữ output path của yt-dlp ổn định với thư mục rất dài hoặc có Unicode, đồng thời khôi phục filepath hoàn tất để Library/history được ghi đúng

## [0.19.1-custom.6] - 2026-07-05

### Sửa lỗi
- **Profile xác thực Firefox** - Chuyển tên hiển thị cũ như `default-release` sang đúng thư mục profile thật trước khi yt-dlp đọc cookie trình duyệt
- **Churn format trên Windows** - Chuẩn hóa file text của repo về LF để Biome và build không ghi lại line ending ngoài phạm vi thay đổi

## [0.19.1-custom.5] - 2026-07-04

### Sửa lỗi
- **Xác nhận khi xoá** - Dùng hộp thoại xác nhận trong app cho thao tác xoá toàn bộ Library và Logs
- **Tên tải xuống Unicode** - Giữ tên tiếng Việt và Unicode từ metadata hoặc filepath cuối thay vì stdout Windows bị mất ký tự

### Thay đổi
- **README fork custom** - Trình bày rõ public fork và kênh release custom cho tester

## [0.19.1-custom.4] - 2026-07-03

### Sửa lỗi
- **Text dán có URL** - Tách URL hợp lệ từ đoạn text dán vào Download, Universal Download, Gallery và Metadata
- **Mở trong thư mục** - Chuyển các action reveal file qua command `open_file_location` của app để ổn định hơn trên Windows
- **Schema test history** - Giữ test database history tương thích với schema in-memory cũ khi cần các cột metadata mới

## [0.19.1-custom.3] - 2026-07-03

### Thay đổi
- **Tự động hóa release** - Giữ workflow build release của source custom trên GitHub release action v3 đang được duy trì
- **Đóng gói extension release** - Cho phép build tag ở source bỏ qua bước ký Firefox khi chưa cấu hình AMO credentials
- **Đóng gói release Windows** - Chỉ build NSIS trong workflow release source để version prerelease custom không làm MSI packaging fail

## [0.19.1-custom.2] - 2026-07-02

### Thêm mới
- **Tách chapter có sẵn** - Thêm cài đặt tùy chọn để tách chapter có sẵn trong video thành file riêng, có thể đánh số theo thứ tự chapter

### Thay đổi
- **Collection tự động trong Thư viện** - Đưa cả file chapter đã tách vào collection tự động khi cài đặt này được bật

## [0.19.1-custom.1] - 2026-07-02

### Thêm mới
- **Tách media** - Thêm action trong Thư viện để tách file đã tải thành các segment mà không tự tạo collection ngoài ý muốn
- **Collection tự động trong Thư viện** - Thêm cài đặt tùy chọn để nhóm playlist đã tách và lượt tải từ channel vào collection trong Thư viện

### Thay đổi
- **Version updater custom** - Chuyển bản custom sang `0.19.1-custom.1` để updater custom phân biệt với `0.19.0-custom.1`

### Sửa lỗi
- **Dialog summary trong Thư viện** - Giữ nội dung summary đã lưu dễ đọc hơn bằng điều khiển cỡ chữ trong dialog

## [0.19.0] - 2026-07-01

### Thêm mới
- **Folder lưu từng item trong hàng đợi** - Thêm action trên item để chọn folder tải riêng cho từng lượt tải
- **Đánh số hàng đợi và playlist** - Thêm tùy chọn prefix tên file theo thứ tự hàng đợi hoặc playlist
- **Phát hiện tải trùng** - Kiểm tra bản ghi Library/history trước khi thêm lượt tải, với lựa chọn hỏi, bỏ qua hoặc vẫn thêm
- **Cách xóa trong Library** - Thêm cài đặt chỉ xóa bản ghi Library hoặc xóa cả file media
- **Ưu tiên tải 30 FPS** - Thêm tùy chọn ưu tiên 30 FPS cho Download, Universal Download và Channels
- **Nút xóa ô nhập** - Thêm nút xóa nhanh cho ô URL ở Download, Universal Download và Gallery

### Thay đổi
- **Tải dữ liệu kênh** - Thêm action dừng fetch và tránh fetch lại không cần thiết khi quay lại kênh vừa duyệt
- **Đổi folder hàng đợi** - Hỏi xác nhận trước khi áp dụng folder tải global mới cho các item đang chờ
- **Link release custom** - Giữ updater, link tải extension và link GitHub trong app ở kênh custom `anhtahaylove/youwee`

### Sửa lỗi
- **Profile xác thực Firefox** - Dùng đúng thư mục profile Firefox thực tế khi truyền profile đã phát hiện cho yt-dlp
- **Bộ lọc match của yt-dlp** - Xem lượt tải bị yt-dlp match filter bỏ qua là lỗi skipped không cần retry
- **Popup AI Summary trong extension** - Mở Summary qua content script của tab hiện tại trước, có fallback bằng deep link trực tiếp
- **Deep link extension khi app chưa mở** - Giữ link tải đang chờ từ extension khi Youwee được mở từ trạng thái chưa chạy
- **Danh sách đánh số trong AI Summary** - Giữ thứ tự đánh số liên tục khi render summary đã lưu
- **Icon trong Settings** - Giữ kích thước Font Awesome icon đồng bộ với chữ bên cạnh

## [0.18.0] - 2026-06-29

### Thêm mới
- **AI Summary trong extension** - Thêm nút Tóm tắt trong browser extension để mở video YouTube trực tiếp ở màn AI Summary
- **Giới hạn token cho AI Summary** - Thêm ô tùy chọn trong Cài đặt để chỉnh số token đầu ra tối đa khi tạo bản tóm tắt
- **Quy tắc bỏ qua cookie** - Thêm quy tắc theo site để bỏ qua xác thực cookie cho URL khớp, mặc định gồm Facebook Reels
- **YouTube player client** - Thêm cài đặt player client của yt-dlp để xử lý lỗi YouTube 403 và lỗi chọn định dạng

### Thay đổi
- **Giao diện extension** - Làm mới popup và menu nút nổi của browser extension theo phong cách gọn, hiện đại và đồng bộ hơn với trình phát nhạc

### Sửa lỗi
- **Nút nổi trong extension** - Sửa lỗi nút nổi của browser extension không hiện hoặc bị crash trên các tab đã mở sẵn trước khi extension được cài hoặc reload
- **Tải từ extension khi app chưa mở** - Sửa lỗi bấm `Download now` trong browser extension chỉ mở Youwee nhưng không thêm video khi app desktop chưa chạy sẵn
- **Thông tin video cho AI Summary** - Sửa lỗi AI Summary bị kẹt khi lấy thông tin video trong trường hợp phụ đề có sẵn nhưng yt-dlp không chọn được định dạng video
- **Độ dài AI Summary** - Bỏ giới hạn token đầu ra mặc định bị hard-code để provider dùng mặc định của model, trừ khi người dùng tự đặt giá trị

## [0.17.2] - 2026-06-17

### Thay đổi
- **Font và icon giao diện** - Đóng gói sẵn font và icon để giao diện hiển thị ổn định mà không phụ thuộc vào CDN bên ngoài
- **Bố cục chi tiết plugin** - Đổi phần chi tiết plugin đã import trong Cài đặt sang dạng tab cho Thông tin, Quyền được yêu cầu, và Runtime & tương thích
- **Nút tải thêm trong Kênh** - Đổi nút Tải thêm ở màn hình duyệt Kênh và chi tiết kênh đã theo dõi sang kiểu floating giống Tìm YouTube theo từ khóa

### Sửa lỗi
- **Ẩn hiện trình phát nhạc** - Sửa lỗi trình phát nhạc khi thu gọn vẫn để lại lớp phủ vô hình, có thể chặn nút bên dưới và làm nhạc phát lại từ đầu khi người dùng bấm vào vùng bị che
- **Trạng thái hoàn tất của Kênh** - Sửa lỗi video trong kênh đã theo dõi bị mất trạng thái hoàn tất sau khi mở lại app bằng cách lưu chắc trạng thái tải thủ công và khôi phục theo đúng video ID cùng file lịch sử còn tồn tại
- **Theo dõi trùng kênh** - Trả về record kênh đã theo dõi hiện có khi follow lại cùng URL để tránh đồng bộ video vào channel id không tồn tại

## [0.17.1] - 2026-06-11

### Thêm mới
- **Lên lịch cho live sắp bắt đầu** - Thêm action trực tiếp trên item trong hàng đợi để lên lịch thử tải lại khi live YouTube chưa bắt đầu
- **Nguồn từ khóa YouTube trong Xuất dữ liệu** - Thêm nguồn từ khóa YouTube để export kết quả tìm kiếm từ Data Export
- **Folder lưu từ CLI** - Thêm `--output` / `-o` để mỗi lượt tải được thêm từ CLI có thể dùng folder lưu tuyệt đối riêng

### Thay đổi
- **Điều khiển lên lịch** - Tinh chỉnh popover lên lịch và trạng thái lịch đang chạy với preset nhanh hơn, preview rõ hơn và countdown gọn hơn
- **Cài đặt và tài liệu CLI** - Tinh chỉnh card CLI trong General settings, bổ sung bản dịch CLI cho toàn bộ ngôn ngữ được hỗ trợ, và mở rộng hướng dẫn CLI với ghi chú cài đặt cho macOS, Windows và Linux

### Sửa lỗi
- **Cập nhật FFmpeg app-managed** - Lưu version release của gói FFmpeg app-managed đã cài để check update không còn lặp khi binary FFmpeg báo version build git nội bộ
- **Trạng thái FFmpeg** - Hiển thị FFmpeg hệ thống được tự phát hiện là System thay vì App managed, và báo lỗi verify cài đặt app-managed rõ hơn
- **Phiên bản yt-dlp bundled** - Ưu tiên sidecar đi kèm bản app hiện tại để binary app-managed cũ không che bản bundled mới hơn
- **Nguồn yt-dlp trong Channel** - Sửa luồng duyệt Channel để dùng đúng nguồn/channel yt-dlp đã chọn, gồm Stable và System
- **URL paste bị escape** - Chuẩn hóa dấu câu URL bị shell escape trong ô nhập Download, Universal và Gallery để link dạng `watch\?v\=...` vẫn thêm đúng video vào hàng đợi
- **URL CLI bị escape** - Chuẩn hóa dấu câu URL bị shell escape để URL YouTube có dạng `watch\?v\=...` vẫn tải đúng video mong muốn
- **Lỗi live đã lên lịch** - Hiển thị thông báo rõ ràng cho live YouTube sắp diễn ra thay vì gộp vào lỗi skipped hoặc unspecified chung chung
- **Output CLI trên Windows** - Sửa lỗi `youwee -V` và `youwee --help` không in kết quả trong terminal Windows

## [0.17.0] - 2026-06-07

### Thêm mới
- **Giao diện dòng lệnh** - Thêm CLI local `youwee` với nút cài đặt, request tải có cấu trúc và các tùy chọn chất lượng, chế độ âm thanh, chỉ thêm vào queue, playlist, phụ đề, cắt đoạn tải, tải live từ đầu và bỏ qua live
- **Bỏ qua live** - Thêm cài đặt tải xuống để bỏ qua video đang phát trực tiếp trong giao diện YouTube và Universal
- **Tìm kiếm tóm tắt AI** - Thêm tìm kiếm Thư viện dùng SQLite FTS5 trên tiêu đề, URL, đường dẫn file và các bản tóm tắt AI đã lưu, kèm lựa chọn phạm vi tìm trong tất cả nội dung, chỉ chi tiết hoặc chỉ tóm tắt AI
- **Tìm YouTube theo từ khóa** - Thêm màn hình tìm video YouTube theo từ khóa riêng với bộ lọc ngày tải lên, thời lượng, thứ tự ưu tiên và tính năng video, cho phép chọn kết quả rồi thêm trực tiếp vào hàng đợi tải
- **Tab trạng thái hàng đợi** - Thêm tab trạng thái gọn cho hàng đợi YouTube và Universal để lọc video theo trạng thái tải
- **Cầu nối tìm kiếm YouTube cho Plugin SDK** - Mở app-managed YouTube keyword search cho plugin JavaScript qua `ctx.youwee.youtube.searchVideos(...)`, kèm bộ lọc có kiểu dữ liệu rõ ràng và hỗ trợ continuation

### Thay đổi
- **Thời gian chờ AI** - Mở rộng lựa chọn Thời gian chờ khi tạo AI lên tối đa 60 phút và áp dụng timeout đã chọn vào HTTP request của các nhà cung cấp AI để hỗ trợ tóm tắt video dài
- **Tên file database của ứng dụng** - Đổi database SQLite nội bộ từ `logs.db` sang `youwee.db`, tự động migrate từ file cũ và giữ file cũ làm backup

### Sửa lỗi
- **Bilibili HTTP 412** - Thêm header giống trình duyệt cho request Bilibili qua yt-dlp để tránh lỗi `HTTP Error 412`

## [0.16.0] - 2026-06-02

### Thêm mới
- **Lưu hàng đợi tải xuống** - Thêm tùy chọn trong cài đặt Tải xuống để lưu các item trong queue YouTube, Universal và Gallery vào database của ứng dụng, giúp khôi phục lại hàng đợi sau khi đóng và mở lại Youwee
- **Xuất dữ liệu** - Thêm không gian Xuất dữ liệu mới để xuất danh sách từ playlist và kênh YouTube, chọn chính xác các cột cần lấy, lưu file ở nhiều định dạng như CSV, Excel, JSON, Markdown, HTML, SQLite và Word, đồng thời lưu file đã xuất vào Thư viện để mở lại sau
- **Tải từ xa qua Telegram** - Thêm mục cài đặt Remote Download với điều khiển Telegram bằng long polling, nhập chat ID được phép dạng tag, popup hướng dẫn lệnh, hỗ trợ `/add`, `/download`, `/status`, `/queue`, `/stop`, `/help`, cùng cú pháp chất lượng ngắn như `720`, `audio`, và `mp3`

### Thay đổi
- **Thêm vào queue khi đang tải** - Cho phép thêm URL mới vào queue YouTube, Universal và Gallery trong lúc đang tải, đồng thời worker chờ ngắn để nhận item vừa thêm trước khi kết thúc phiên tải hiện tại
- **Chọn định dạng YouTube** - Đổi codec video mặc định của YouTube sang Auto để lượt tải mới không còn ép chọn riêng H.264 và đồng nhất hơn với Universal khi video không có stream AVC phù hợp

### Sửa lỗi
- **Xung đột cài đặt deb trên Linux** - Đổi tên yt-dlp bundled sang tên binary riêng của Youwee để gói `.deb` không còn đụng với package `yt-dlp` do distro quản lý
- **Đường dẫn dependency hệ thống** - Cải thiện resolve PATH trên Windows cho yt-dlp, FFmpeg, Deno, gallery-dl và các công cụ phụ trợ
- **Chọn profile cookie Firefox** - Ưu tiên profile Firefox đang active từ `profiles.ini` để tải bằng cookie trình duyệt dùng đúng profile có khả năng đang lưu cookie

## [0.15.1] - 2026-05-27

### Thay đổi
- **Cải tiến UI và UX** - Hoàn thiện giao diện và trải nghiệm trên AI Features, metadata, phần cài đặt plugin, dialog guide, và hệ thống thông báo dùng chung để trải nghiệm Youwee đồng nhất hơn
- **Hỗ trợ Plugin SDK v2.0.0** - Nâng cấp phần plugin lên `youwee-sdk` `v2.0.0`, gồm permission `read/write/AI` chặt chẽ hơn và hỗ trợ workspace plugin ưu tiên TypeScript

## [0.14.1] - 2026-05-24

### Thay đổi
- **Luồng hướng dẫn plugin và cấp quyền** - Tinh chỉnh luồng import plugin, duyệt quyền, hướng dẫn workspace, và giao diện cấu hình để việc cài plugin, gán workflow, và đọc guide trong Settings rõ ràng hơn
- **Tăng độ linh hoạt cho plugin author** - Cập nhật tích hợp workspace và SDK để hỗ trợ icon Lucide linh hoạt hơn và luồng test Deno gọn hơn cho người viết plugin

### Sửa lỗi
- **Log runtime plugin và tóm tắt output** - Giảm log plugin bị lặp, bỏ lưu raw protocol output của kết quả plugin, và hiển thị rõ hơn các đường dẫn file đầu ra sẽ được dùng cho step tiếp theo trong log post-processing
- **Hiển thị guide plugin và tài liệu đa ngôn ngữ** - Sửa lỗi dialog guide bị tràn nội dung và giữ lại các file `README.<locale>.md` cho plugin đã cài
- **Hoàn thiện giao diện cấu hình plugin** - Cải thiện control multi-select, thời điểm hiển thị validation, và các prompt khi import/bật plugin để đồng bộ hơn với giao diện chung của ứng dụng

## [0.14.0] - 2026-05-24

### Thêm mới
- **Tag và bộ sưu tập cho Thư viện** - Thêm tag tự do và bộ sưu tập ảo cho các mục trong Thư viện, bao gồm gán ngay trên từng item, filter nhanh bằng chip, quản lý bộ sưu tập và lọc nâng cao theo tag hoặc bộ sưu tập
- **Hệ thống plugin có ký và luồng SDK** - Thêm plugin `.ywp` có chữ ký với luồng attach/debug workspace, field cấu hình có kiểu dữ liệu rõ ràng, hướng dẫn plugin đa ngôn ngữ, duyệt quyền, gán vào workflow, xem log, và hỗ trợ đóng gói/ký bằng `youwee-sdk`

### Thay đổi
- **Đồng bộ ô nhập URL của Gallery** - Cập nhật ô nhập Gallery để bám sát hơn luồng single/multiple của YouTube, gồm style nút Add, layout batch, hint URL và wording đa ngôn ngữ

### Sửa lỗi
- **Crash khởi động trên Arch Linux với GBM EGL display** - Thêm fallback cho WebKitGTK trên Linux, mặc định tắt GBM renderer để tránh app bị abort khi khởi động trên một số hệ Arch-based

## [0.13.3] - 2026-05-14

### Thêm mới

### Thay đổi

### Sửa lỗi
- **Vòng lặp tải video ở kênh đã theo dõi** - Sửa regression từ tính năng phân trang khiến kênh đã theo dõi có thể bị kẹt trong trạng thái tải liên tục và không lấy danh sách video ổn định
- **Race condition ở progress khi tải kênh** - Progress duyệt kênh giờ bỏ qua event cũ từ request trước khi người dùng đổi kênh hoặc bấm tải lại liên tục
- **Tải thêm làm tăng sai số video mới** - Các video cũ được nạp từ những trang bổ sung của kênh sẽ không còn bị lưu là video mới, tránh sai badge và tray count

## [0.13.2] - 2026-05-10

### Thêm mới
- **Phân trang video kênh với Tải thêm** - Trang Kênh giờ tải mặc định 100 video đầu tiên và cho phép tải thêm theo từng đợt trong cả màn hình duyệt và màn hình chi tiết
- **Hỗ trợ tiếng Thái** - Bổ sung bản địa hóa đầy đủ tiếng Thái cho toàn bộ ứng dụng, bao gồm màn hình giao diện, cài đặt, công cụ phụ đề, luồng tải xuống và bộ chọn ngôn ngữ
- **Hỗ trợ tiếng Ả Rập** - Bổ sung bản địa hóa đầy đủ tiếng Ả Rập cho toàn bộ ứng dụng, bao gồm màn hình giao diện, cài đặt, công cụ phụ đề, luồng tải xuống, đồng thời thêm xử lý hướng chữ RTL

### Thay đổi

### Sửa lỗi
- **Giới hạn duyệt kênh ở 50 video** - Sửa lỗi duyệt kênh và playlist bị dừng sai ở 50 video dù vẫn còn thêm video
- **Phân loại sai dialog lỗi cookie** - Sửa lỗi yêu cầu cookie đăng nhập mới bị hiển thị nhầm như lỗi khóa DB cookie của trình duyệt; giờ app tách riêng dialog cho lỗi DB lock và lỗi cần cookie xác thực

## [0.13.1] - 2026-04-26

### Thêm mới
- **Nhà cung cấp AI LM Studio** - Thêm LM Studio làm nhà cung cấp AI nội bộ tương thích OpenAI, có endpoint local tùy chỉnh và không yêu cầu khóa API

### Thay đổi

### Sửa lỗi
- **Hậu xử lý WebM 4K** - Sửa lỗi tải WebM có thể chọn nhầm stream tương thích MP4/H.264 khiến FFmpeg thất bại ở bước post-processing conversion

## [0.13.0] - 2026-04-15

### Thêm mới
- **Trình phát nhạc nổi** - Thêm player âm thanh trong ứng dụng với hàng đợi, điều khiển phát, tốc độ và âm lượng
- **Tích hợp Aria2 làm trình tải ngoài** - Bổ sung hỗ trợ `aria2c` làm external downloader với tham số tùy chỉnh và xử lý lỗi đã bản địa hóa
- **Đổi tên file đã tải từ Queue và Thư viện** - Thêm thao tác đổi tên sau khi tải xong (queue YouTube + Universal và Thư viện), đồng bộ đường dẫn/tên trong DB và giao diện đa ngôn ngữ
- **Bộ lọc nâng cao và sắp xếp trong Thư viện** - Thêm panel Advanced Filters (loại media, khoảng ngày, định dạng, chất lượng), tìm kiếm theo `title + filepath`, và sắp xếp lịch sử có ghi nhớ lựa chọn sort
- **Trang tải Gallery riêng** - Thêm menu `Gallery` mới nằm dưới Universal, có ô nhập URL riêng, import hàng loạt, hàng đợi, chọn thư mục lưu và luồng Start/Stop dành cho các nguồn kiểu gallery chạy bằng `gallery-dl`

### Thay đổi
- **Xử lý queue động khi đang tải** - Worker queue giờ claim item theo thời gian thực, nên video thêm mới sẽ vào cuối hàng đợi và tự tải tiếp mà không cần bấm Start lại

### Sửa lỗi
- **Ổn định test đổi tên lịch sử trên CI** - Serialize các test dùng chung DB in-memory để tránh lỗi flaky `History entry not found` khi `cargo test` chạy song song
## [0.12.0] - 2026-03-04

### Thêm mới
- **Bộ chọn nguồn dependency (yt-dlp/FFmpeg)** - Thêm tùy chọn trong Cài đặt → Phụ thuộc để chọn dùng binary do ứng dụng quản lý hoặc do hệ thống quản lý
- **Xác nhận an toàn khi chuyển sang nguồn hệ thống** - Thêm hộp thoại xác nhận khi đổi yt-dlp/FFmpeg sang nguồn hệ thống để tránh bấm nhầm

### Thay đổi
- **Nhãn nguồn hệ thống theo hệ điều hành** - Nhãn nguồn hệ thống giờ hiển thị theo nền tảng (`Homebrew` trên macOS, `PATH` trên Windows, trình quản lý gói trên Linux)
- **Tự động tạo ghi chú phát hành GitHub trong luồng build** - Bật `generate_release_notes` trong workflow phát hành để bản phát hành có ghi chú tự sinh
- **Tích hợp thanh tiêu đề tùy chỉnh trên Windows** - Thay title bar native của Windows bằng control tùy chỉnh theo theme ứng dụng (vùng kéo cửa sổ, thu nhỏ/phóng to/đóng)

### Sửa lỗi
- **Ghi lịch sử tải xuống trên Windows** - Bắt chính xác đường dẫn file đầu ra cuối cùng trên Windows để bản tải hoàn tất luôn được thêm vào lịch sử Thư viện
- **Phân tích đường dẫn tải lại trên Windows** - Sửa tách thư mục output khi tải lại để xử lý đúng đường dẫn dùng dấu `\`
- **Xử lý output yt-dlp không phải UTF-8 trên Windows** - Thêm fallback decode GBK/ANSI và xử lý `--print-to-file` để vẫn lấy đúng đường dẫn file ở locale không UTF-8
- **Tự động làm mới Thư viện khi tải xong** - Lịch sử Thư viện giờ tự refresh khi trạng thái tải chuyển sang `finished`
- **Tương thích URL Douyin dạng modal** - Chuẩn hóa URL `douyin.com` có `modal_id` về dạng chuẩn `/video/{id}` trong backend yt-dlp và parser deep-link phía frontend

## [0.11.1] - 2026-03-01

### Thêm mới
- **Hỗ trợ tiếng Pháp, Bồ Đào Nha và Nga** - Bản địa hóa đầy đủ Français, Português và Русский cho toàn bộ giao diện, cài đặt, thông báo lỗi và nhãn metadata
- **Bản địa hóa thông báo lỗi backend** - Các thông báo lỗi từ backend (lỗi tải, lỗi mạng, v.v.) giờ được dịch theo ngôn ngữ người dùng đã chọn thay vì luôn hiển thị tiếng Anh

### Thay đổi
- **Tái cấu trúc chuỗi fallback transcript** - Thống nhất logic fallback transcript giữa AI summary và processing để hành vi nhất quán hơn

### Sửa lỗi
- **Fallback transcript cho Douyin và TikTok** - Cải thiện trích xuất transcript cho video Douyin và TikTok trước đây bị thất bại im lặng
- **Lỗi transcript và caption ngắn** - Lỗi transcript giờ được giữ lại để chẩn đoán thay vì bị nuốt im lặng; caption ngắn được chấp nhận là transcript hợp lệ thay vì bị từ chối
- **Cài đặt mặc định TikTok** - Điều chỉnh cài đặt tải mặc định của TikTok cho phù hợp với quy ước nền tảng

## [0.11.0] - 2026-02-20

### Thêm mới
- **Browser Extension tải nhanh (Chromium + Firefox)** - Giờ đây bạn có thể gửi trang video đang mở từ trình duyệt sang Youwee và chọn `Download now` hoặc `Add to queue`
- **Thiết lập Extension trong Cài đặt** - Thêm mục mới Cài đặt → Extension với nút tải trực tiếp và hướng dẫn cài đơn giản cho Chromium và Firefox

### Thay đổi
- **Làm mới UI/UX cho trang YouTube và Universal** - Tối giản thao tác nhập link, card preview, hàng đợi và phần title bar để giao diện gọn và đồng nhất hơn

### Sửa lỗi
- **Đồng bộ resolve dependency giữa các tính năng** - Chuẩn hóa luồng chọn yt-dlp/FFmpeg trong download, metadata, channels và polling nền để luôn tôn trọng nguồn đã chọn
- **Chế độ system fail rõ ràng khi thiếu binary** - Khi chọn nguồn hệ thống mà thiếu binary, ứng dụng giờ báo lỗi rõ ràng thay vì fallback ngầm

## [0.10.1] - 2026-02-15

### Thêm mới
- **Thiết lập font ASS** - Thêm tùy chỉnh font và cỡ chữ phụ đề cho xuất ASS và preview
- **Luồng xuống dòng phụ đề** - Thêm thao tác auto xuống dòng nhanh và hỗ trợ Shift+Enter khi chỉnh nội dung
- **Tự động thử lại có thể cấu hình** - Thêm cài đặt Auto Retry cho tải YouTube và Universal, cho phép đặt số lần thử lại và thời gian chờ để tự phục hồi khi mạng không ổn định hoặc live stream bị ngắt

### Thay đổi

### Sửa lỗi
- **Thông báo lỗi tải xuống rõ hơn** - Cải thiện thông báo lỗi yt-dlp với nguyên nhân cụ thể hơn để hỗ trợ nhận diện lỗi tạm thời và thử lại tự động chính xác hơn

## [0.10.0] - 2026-02-15

### Thêm mới
- **Xưởng phụ đề** - Thêm trang phụ đề tất cả trong một cho SRT/VTT/ASS với chỉnh sửa nội dung, công cụ thời gian, tìm/thay thế, tự sửa lỗi và các tác vụ AI (Whisper, Dịch, Sửa ngữ pháp)
- **Bộ công cụ phụ đề nâng cao** - Bổ sung timeline sóng âm/phổ tần, đồng bộ theo cảnh cắt, QC realtime theo style profile, công cụ tách/gộp, chế độ Dịch 2 cột (gốc/bản dịch), và công cụ batch cho project

### Thay đổi

### Sửa lỗi

## [0.9.4] - 2026-02-14

### Thêm mới
- **Chọn thư mục output cho Processing** - Thêm nút chọn thư mục lưu đầu ra trong khung chat Processing. Mặc định vẫn là thư mục của video chính, và output của AI/quick actions sẽ theo thư mục đã chọn
- **Đính kèm nhiều loại file trong chat AI Processing** - Chat Processing hỗ trợ đính kèm ảnh/video/phụ đề (chọn file + kéo thả), hiển thị preview và metadata phù hợp theo từng loại
- **Lối tắt đề xuất ngôn ngữ trong Cài đặt** - Thêm link nhanh trong Cài đặt → Chung để người dùng bình chọn/đề xuất ngôn ngữ tiếp theo trên GitHub Discussions
- **Kiểm tra cập nhật app từ system tray** - Thêm hành động mới trong tray để kiểm tra cập nhật Youwee trực tiếp

### Thay đổi
- **Sinh lệnh subtitle/merge ổn định hơn** - Luồng tạo lệnh Processing ưu tiên xử lý deterministic cho chèn phụ đề và ghép nhiều video (bao gồm gợi ý thứ tự intro/outro) trước khi fallback sang AI
- **Đổi tên mục kiểm tra kênh trong tray cho rõ nghĩa** - Đổi "Kiểm tra tất cả" thành "Kiểm tra kênh theo dõi ngay" để thể hiện đúng hành vi kiểm tra các kênh đã theo dõi
- **Đơn giản hóa tiêu đề trang** - Bỏ icon phía trước tiêu đề ở các trang Metadata, Processing và AI Summary để giao diện gọn hơn

### Sửa lỗi
- **Lỗi lấy thông tin video khi dùng xác thực/proxy** - Sửa thứ tự tham số yt-dlp để cờ cookie và proxy được chèn trước dấu phân tách URL `--`, tránh lỗi `Failed to fetch video info` trong khi luồng tải video vẫn hoạt động đúng
- **Kênh Stable luôn báo có bản cập nhật** - Sửa logic kiểm tra cập nhật yt-dlp cho stable/nightly để đọc phiên bản thực từ binary đã cài (`--version`) thay vì chỉ dựa vào metadata tồn tại file, giúp hiển thị đúng trạng thái "Đã cập nhật" sau khi cập nhật xong
- **Trạng thái cập nhật Bundled và binary đang dùng không đồng bộ** - Sửa luồng cập nhật bundled để hiển thị phiên bản mới có sẵn trong Settings và ưu tiên dùng binary `app_data/bin/yt-dlp` đã cập nhật khi có, giúp cập nhật bundled có hiệu lực thực tế
- **Làm mới phần thông tin video ở trang Processing** - Thiết kế lại khu vực dưới player theo kiểu YouTube với tiêu đề nổi bật và chip metadata hiện đại, đồng thời bỏ đổi màu hover và shadow ở badge codec để giao diện gọn hơn
- **Dropdown Prompt Templates không tự đóng** - Sửa dropdown Prompt Templates ở Processing để tự đóng khi click ra ngoài hoặc nhấn phím Escape
- **Hiển thị trùng số URL ở Universal** - Sửa badge số lượng URL trong ô nhập Universal bị lặp số (ví dụ `1 1 URL`)

## [0.9.3] - 2026-02-14

### Thêm mới
- **Tải phụ đề trong Metadata** - Thêm nút chuyển đổi phụ đề trong thanh cài đặt Metadata để tải phụ đề (thủ công + tự động tạo) cùng với metadata. Bao gồm popover để chọn ngôn ngữ và định dạng (SRT/VTT/ASS)

### Thay đổi
- **Cải thiện UX nhập thời gian cắt video** - Thay thế ô nhập text thường bằng ô nhập tự động định dạng, tự chèn `:` khi gõ (ví dụ `1030` → `10:30`, `10530` → `1:05:30`). Placeholder thông minh hiển thị `M:SS` hoặc `H:MM:SS` dựa theo độ dài video. Kiểm tra realtime với viền đỏ khi định dạng sai hoặc thời gian bắt đầu >= kết thúc. Hiện tổng thời lượng video khi có

## [0.9.2] - 2026-02-13

### Thêm mới
- **Tải video theo phân đoạn thời gian** - Chỉ tải một đoạn video bằng cách đặt thời gian bắt đầu và kết thúc (ví dụ: 10:30 đến 14:30). Có thể cài đặt cho từng video trên cả hàng đợi YouTube và Universal qua biểu tượng kéo. Sử dụng `--download-sections` của yt-dlp
- **Tự động kiểm tra cập nhật FFmpeg khi khởi động** - Kiểm tra cập nhật FFmpeg giờ chạy tự động khi mở app (cho bản cài đặt tích hợp). Nếu có bản cập nhật, sẽ hiển thị trong Cài đặt > Phụ thuộc mà không cần bấm nút làm mới

## [0.9.1] - 2026-02-13

### Sửa lỗi
- **Ứng dụng crash trên macOS không có Homebrew** - Sửa lỗi crash khi khởi động do thiếu thư viện động `liblzma`. Crate `xz2` giờ dùng static linking, giúp ứng dụng hoàn toàn độc lập không cần Homebrew hay thư viện hệ thống
- **Tự động tải bỏ qua cài đặt người dùng** - Tự động tải kênh giờ áp dụng cài đặt riêng cho mỗi kênh (chế độ Video/Âm thanh, chất lượng, định dạng, codec, bitrate) thay vì dùng giá trị mặc định. Mỗi kênh có cài đặt tải riêng có thể cấu hình trong bảng cài đặt kênh
- **Tăng cường bảo mật** - FFmpeg giờ dùng mảng tham số thay vì parse chuỗi shell, chặn command injection. Thêm validate URL scheme và `--` separator cho mọi lệnh yt-dlp để chặn option injection. Bật Content Security Policy, xóa quyền shell thừa, và thêm `isSafeUrl` cho các link hiển thị
- **Lỗi preview video với container MKV/AVI/FLV/TS** - Phát hiện preview giờ kiểm tra cả container và codec. Video trong container không hỗ trợ (MKV, AVI, FLV, WMV, TS, WebM, OGG) được tự động transcode sang H.264. HEVC trong MP4/MOV không còn bị transcode thừa trên macOS
- **Hẹn giờ tải không hiển thị khi thu nhỏ vào tray** - Thông báo desktop giờ hiển thị khi tải hẹn giờ bắt đầu, dừng hoặc hoàn thành trong khi ứng dụng thu nhỏ vào system tray. Menu tray hiển thị trạng thái hẹn giờ (vd: "YouTube: 23:00"). Hẹn giờ hoạt động trên cả trang YouTube và Universal
- **Thoát từ tray hủy download đang chạy** - Nút "Thoát" trên tray giờ dùng tắt an toàn thay vì kill process, cho phép download đang chạy hoàn tất cleanup và tránh file bị hỏng
- **Cài đặt ẩn Dock bị mất khi khởi động lại (macOS)** - Tùy chọn "Ẩn biểu tượng Dock khi đóng" giờ được đồng bộ với native layer khi khởi động app, không chỉ khi vào trang Cài đặt
- **Hàng đợi Universal hiện skeleton thay vì URL khi đang tải** - Thay thế placeholder skeleton nhấp nháy bằng URL thực tế và badge spinner "Đang tải thông tin...". Khi lấy metadata thất bại, item giờ thoát trạng thái loading thay vì hiện skeleton mãi mãi

## [0.9.0] - 2026-02-12

### Thêm mới
- **Theo dõi kênh & Tải tự động** - Theo dõi các kênh YouTube, duyệt video, chọn và tải hàng loạt với đầy đủ tùy chọn chất lượng/codec/định dạng. Polling nền phát hiện video mới với thông báo desktop và badge đếm video mới theo kênh. Panel kênh theo dõi thu gọn được, hỗ trợ thu nhỏ xuống system tray
- **Xác nhận xem trước file lớn** - Ngưỡng kích thước file có thể cấu hình (mặc định 300MB) hiển thị hộp thoại xác nhận trước khi tải video lớn trong trang Xử lý. Điều chỉnh ngưỡng tại Cài đặt → Chung → Xử lý
- **Tìm kiếm cài đặt đa ngôn ngữ** - Tìm kiếm trong cài đặt giờ hoạt động với mọi ngôn ngữ. Tìm bằng tiếng Việt (ví dụ "giao diện") hoặc tiếng Trung đều cho kết quả. Từ khóa tiếng Anh vẫn hoạt động như dự phòng

### Sửa lỗi
- **Trang Xử lý bị trắng màn hình với video 4K VP9/AV1/HEVC (Linux)** - Bộ giải mã AAC của GStreamer gây crash WebKitGTK khi phát video VP9/AV1/HEVC. Preview giờ dùng phương pháp dual-element: video H.264 không âm thanh + file WAV riêng biệt đồng bộ qua JavaScript, hoàn toàn bỏ qua đường dẫn AAC bị lỗi. Nếu phát video vẫn thất bại, tự động chuyển sang ảnh thu nhỏ JPEG tĩnh. Hoạt động trên macOS, Windows và Linux

## [0.8.2] - 2026-02-11

### Thêm mới
- **Ghi chú cập nhật đa ngôn ngữ** - Hộp thoại cập nhật hiển thị ghi chú phát hành theo ngôn ngữ người dùng (Tiếng Anh, Tiếng Việt, Tiếng Trung). CI tự động trích xuất nhật ký thay đổi từ các file CHANGELOG theo ngôn ngữ
- **Tùy chọn chất lượng 8K/4K/2K cho Universal** - Dropdown chất lượng giờ có thêm 8K Ultra HD, 4K Ultra HD và 2K QHD, giống như tab YouTube. Tự động chuyển sang chất lượng cao nhất có sẵn nếu nguồn không hỗ trợ
- **Nút bật/tắt "Phát từ đầu" cho Universal** - Nút mới trong Cài đặt nâng cao để ghi live stream từ đầu thay vì từ thời điểm hiện tại. Sử dụng flag `--live-from-start` của yt-dlp
- **Xem trước video cho Universal** - Tự động hiển thị thumbnail, tiêu đề, thời lượng và kênh khi thêm URL từ TikTok, Bilibili, Facebook, Instagram, Twitter và các trang khác. Thumbnail cũng được lưu vào Thư viện
- **Nhận diện nền tảng thông minh hơn** - Thư viện giờ nhận diện và gắn nhãn chính xác hơn 1800 trang web được yt-dlp hỗ trợ (Bilibili, Dailymotion, SoundCloud, v.v.) thay vì hiển thị "Khác". Thêm tab lọc Bilibili

### Sửa lỗi
- **Trang Xử lý bị treo khi upload video (Linux)** - File video được đọc toàn bộ vào RAM qua `readFile()`, gây tràn bộ nhớ và màn hình trắng. Giờ sử dụng giao thức asset của Tauri để stream video trực tiếp mà không cần tải vào bộ nhớ. Thêm Error Boundary để ngăn màn hình trắng, xử lý lỗi video với thông báo cụ thể theo codec, dọn dẹp blob URL chống rò rỉ bộ nhớ, và nhận dạng MIME type đúng cho các định dạng không phải MP4
- **Thumbnail bị lỗi trong Thư viện** - Sửa thumbnail từ các trang như Bilibili sử dụng URL HTTP. Thumbnail giờ hiển thị biểu tượng thay thế khi không tải được
- **Thư viện không làm mới khi chuyển trang** - Thư viện giờ tự động tải dữ liệu mới nhất khi chuyển đến trang thay vì phải làm mới thủ công
