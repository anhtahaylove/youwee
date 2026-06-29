# PRD ngắn: Plugin Fallback Retry/Recovery

## Mục tiêu
Quyết định có giữ plugin fallback retry/recovery như một năng lực chính thức hay không, sau khi Youwee custom đã chuyển Facebook Reels fallback vào core download path.

## Bối cảnh
- Core fallback hiện là hướng chính cho Facebook Reels và phải ghi Library/history chuẩn.
- Plugin `without-cookie-fallback` local đã bị xóa; registry legacy có thể còn trong app data nhưng không còn là đường phát triển chính.
- Upstream `v0.18.0` có patch retry/recovery cho plugin fallback, nhưng kéo theo SDK/runtime/plugin workflow surface.

## Người dùng hưởng lợi
- Người dùng cần plugin bên thứ ba phục hồi download thất bại ngoài core path.
- Người phát triển plugin cần retry workflow ổn định và có tín hiệu recovered rõ ràng.

## Non-goals
- Không hồi sinh Facebook Reels plugin fallback.
- Không đổi core fallback hiện tại.
- Không tự bật lại plugin legacy trong app data.

## Phạm vi nếu port
- Plugin retry phải refresh workflow assignment khi retry.
- Plugin recovered event phải cập nhật queue item thành success thay vì giữ failed.
- SDK type/readme/changelog phải khớp event mới.
- UI queue không được đánh dấu failed nếu plugin recovery thành công.

## Tiêu chí quyết định
Port nếu có ít nhất một plugin fallback được giữ làm supported path và có test case thực tế.
Không port nếu plugin fallback chỉ còn là legacy cleanup target.

## Khuyến nghị hiện tại
Không port ngay. Giữ core fallback là đường chính; chỉ lập lane plugin khi quyết định hỗ trợ plugin recovery như sản phẩm riêng.

## Test bắt buộc nếu port
- Unit test plugin recovered mutation.
- SDK test cho event/type mới.
- UI retry test: failed item retry nhận workflow mới và success item không còn failed.
- Manual test với một plugin local không liên quan Facebook Reels.
