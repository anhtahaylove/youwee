import { describe, expect, test } from 'bun:test';
import {
  BACKEND_ERROR_PREFIX,
  extractBackendError,
  inferBackendErrorCode,
  isNonRetryableBackendError,
} from '../src/lib/backend-error';

describe('backend error classification', () => {
  test('treats yt-dlp match filter skips as non-retryable', () => {
    const message = '[download] abc123 does not pass filter: title check';

    expect(inferBackendErrorCode(message)).toBe('YT_SKIPPED_FILTER');
    expect(isNonRetryableBackendError(message)).toBe(true);
  });

  test('unwraps nested backend wire errors before they reach the UI', () => {
    const inner = `${BACKEND_ERROR_PREFIX}${JSON.stringify({
      code: 'NETWORK_TIMEOUT',
      message: 'Timed out inspecting TikTok Live metadata.',
      retryable: true,
    })}`;
    const outer = `${BACKEND_ERROR_PREFIX}${JSON.stringify({
      code: 'BACKEND_UNKNOWN',
      message: inner,
      retryable: false,
    })}`;

    expect(extractBackendError(new Error(outer))).toEqual({
      code: 'NETWORK_TIMEOUT',
      message: 'Timed out inspecting TikTok Live metadata.',
      retryable: true,
    });
  });
});
