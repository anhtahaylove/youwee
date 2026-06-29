import { describe, expect, test } from 'bun:test';
import { inferBackendErrorCode, isNonRetryableBackendError } from '../src/lib/backend-error';

describe('backend error classification', () => {
  test('treats yt-dlp match filter skips as non-retryable', () => {
    const message = '[download] abc123 does not pass filter: title check';

    expect(inferBackendErrorCode(message)).toBe('YT_SKIPPED_FILTER');
    expect(isNonRetryableBackendError(message)).toBe(true);
  });
});
