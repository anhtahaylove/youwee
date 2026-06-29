# Đánh giá: Context useMemo Refactor

## Kết luận
Chưa nên port mặc định. Đây là refactor performance, không phải bugfix; rủi ro chính là stale state trong queue contexts.

## Upstream liên quan
- `7b4c654 feat: optimize context values using useMemo for performance improvements`

## Rủi ro
- `DownloadContext` và `UniversalContext` có nhiều ref/state callback phụ thuộc queue item hiện tại; memo thiếu dependency có thể làm start/retry/cancel đọc state cũ.
- `GalleryDlContext`, `MetadataContext`, `PlayerContext` ít rủi ro hơn nhưng vẫn có callback closure dễ stale.
- Custom đã thêm queue numbering, output folder per item, Facebook Reel metadata và source normalization; cherry-pick refactor dễ làm mất dependency custom.

## Khi nào đáng port
- Có bằng chứng re-render gây chậm UI queue hoặc React profiler cho thấy context value churn là bottleneck.
- Sau khi các flow Download/Universal đã ổn định và không còn patch core queue lớn đang mở.

## Cách port an toàn
- Port từng context một, không gom cả commit.
- Bắt đầu từ context ít state phụ thuộc nhất.
- Mỗi context chỉ memoize object export cuối cùng; không đổi logic callback nếu chưa có test.
- Không cherry-pick trực tiếp nếu dependency list khác custom.

## Test plan trước khi port
- `bun run tsc -b`
- `bun run biome check --write .`
- Existing Bun tests: `bun test`
- UI smoke test Download queue: add URL, fetch metadata, start, cancel, retry, rename, output folder.
- UI smoke test Universal queue: Facebook Reel fallback preview/download, TikTok/Instagram add queue, source badge/history.
- Gallery/Metadata smoke test nếu context tương ứng bị đụng.

## Recommendation
Để sau. Nếu muốn làm, tạo branch/commit riêng chỉ cho một context đầu tiên và đo trước/sau bằng React profiler hoặc log render count.
