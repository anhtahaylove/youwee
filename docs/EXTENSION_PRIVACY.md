# Youwee Browser Extension Privacy Notice

Last updated: 2026-07-17

[Tiếng Việt](#thông-báo-quyền-riêng-tư-của-tiện-ích-youwee)

## Scope

This notice covers the Youwee browser extension for Firefox and Chromium-based browsers. The extension is a companion to the free, open-source Youwee desktop application.

## Data the extension handles

The extension handles only the data needed to send a user-selected media page to Youwee:

- The URL of the active page when the user clicks **Download now**, **Add to queue**, **AI Summary**, or a floating-button action.
- The download choices selected by the user, such as video or audio, quality, and queue action.
- Page layout information processed locally to place the optional floating button near a supported media player.
- Local interface preferences, such as whether the floating button is enabled or collapsed.

The extension does not read browser cookies, passwords, form entries, payment details, or private messages.

## How data is used and transmitted

When the user explicitly starts an action, the extension passes the page URL and selected options to the locally installed Youwee desktop application through the operating system's `youwee://` protocol handler.

The extension does not send browsing data to an analytics service, advertising network, or developer-operated tracking server. It does not sell or share user data for advertising.

After receiving a request, the Youwee desktop application may connect to the source website to fetch metadata or media requested by the user. Optional desktop features, such as AI Summary, may use a provider configured by the user. Those operations occur in the desktop application, not in the browser extension.

## Storage and retention

The extension stores only interface preferences in browser extension storage. It does not keep a browsing-history database or retain submitted URLs.

The Youwee desktop application may keep download history in its local Library. Users can remove individual records or clear that history from the application.

Uninstalling the extension removes its browser-managed local storage. Uninstalling the desktop application and removing its application data removes data held by the desktop application.

## Permissions

- `activeTab`: reads the active page URL after the user opens or clicks the extension.
- `storage`: saves floating-button interface preferences locally.
- `scripting`: restores packaged extension scripts and styles on an already-open supported tab after installation or reload.
- Supported-site access: shows the floating button and processes the current URL locally on explicitly listed media websites.

The extension contains no remotely hosted executable code.

## User control

Users can disable the floating button, remove the extension, decline the operating-system prompt to open Youwee, or avoid starting a transfer. No page URL is sent to Youwee until the user invokes an extension action.

## Contact

For privacy questions or bug reports, open an issue at <https://github.com/anhtahaylove/youwee/issues>.

---

# Thông báo quyền riêng tư của tiện ích Youwee

Cập nhật lần cuối: 2026-07-17

## Phạm vi

Thông báo này áp dụng cho tiện ích Youwee trên Firefox và các trình duyệt nền Chromium. Tiện ích là phần bổ trợ cho ứng dụng desktop Youwee miễn phí và mã nguồn mở.

## Dữ liệu tiện ích xử lý

Tiện ích chỉ xử lý dữ liệu cần thiết để gửi trang media do người dùng chọn sang Youwee:

- URL của trang đang mở khi người dùng bấm **Tải ngay**, **Thêm vào hàng đợi**, **Tóm tắt AI** hoặc thao tác trên nút nổi.
- Lựa chọn tải của người dùng như video/âm thanh, chất lượng và cách xử lý hàng đợi.
- Thông tin bố cục trang được xử lý cục bộ để đặt nút nổi tùy chọn gần trình phát media được hỗ trợ.
- Tùy chọn giao diện cục bộ như bật/tắt hoặc thu gọn nút nổi.

Tiện ích không đọc cookie trình duyệt, mật khẩu, nội dung biểu mẫu, thông tin thanh toán hoặc tin nhắn riêng tư.

## Cách sử dụng và truyền dữ liệu

Chỉ khi người dùng chủ động thực hiện thao tác, tiện ích mới chuyển URL trang và các lựa chọn sang ứng dụng Youwee cài trên cùng máy thông qua giao thức `youwee://` của hệ điều hành.

Tiện ích không gửi hoạt động duyệt web tới dịch vụ phân tích, mạng quảng cáo hoặc máy chủ theo dõi do nhà phát triển vận hành. Dữ liệu người dùng không được bán hoặc chia sẻ cho mục đích quảng cáo.

Sau khi nhận yêu cầu, ứng dụng desktop Youwee có thể kết nối tới website nguồn để lấy metadata hoặc media mà người dùng yêu cầu. Các tính năng desktop tùy chọn như Tóm tắt AI có thể dùng nhà cung cấp do người dùng tự cấu hình. Các thao tác này diễn ra trong ứng dụng desktop, không phải trong tiện ích trình duyệt.

## Lưu trữ và thời gian giữ dữ liệu

Tiện ích chỉ lưu tùy chọn giao diện trong vùng lưu trữ extension của trình duyệt. Tiện ích không tạo cơ sở dữ liệu lịch sử duyệt web và không giữ lại URL đã gửi.

Ứng dụng desktop Youwee có thể lưu lịch sử tải trong Library cục bộ. Người dùng có thể xóa từng bản ghi hoặc xóa lịch sử trong ứng dụng.

Gỡ tiện ích sẽ xóa vùng lưu trữ cục bộ do trình duyệt quản lý. Gỡ ứng dụng desktop và xóa app data sẽ xóa dữ liệu do ứng dụng desktop lưu giữ.

## Quyền truy cập

- `activeTab`: đọc URL của tab đang hoạt động sau khi người dùng mở hoặc bấm tiện ích.
- `storage`: lưu cục bộ tùy chọn giao diện của nút nổi.
- `scripting`: khôi phục script và CSS đóng gói sẵn trên tab được hỗ trợ đã mở trước khi tiện ích được cài hoặc reload.
- Quyền trên các website được hỗ trợ: hiển thị nút nổi và xử lý cục bộ URL hiện tại trên danh sách website media được khai báo rõ.

Tiện ích không chứa mã thực thi được tải từ máy chủ bên ngoài.

## Quyền kiểm soát của người dùng

Người dùng có thể tắt nút nổi, gỡ tiện ích, từ chối hộp thoại mở Youwee của hệ điều hành hoặc không thực hiện thao tác gửi. Không có URL nào được chuyển sang Youwee trước khi người dùng chủ động bấm một hành động của tiện ích.

## Liên hệ

Nếu có câu hỏi về quyền riêng tư hoặc muốn báo lỗi, hãy tạo issue tại <https://github.com/anhtahaylove/youwee/issues>.
