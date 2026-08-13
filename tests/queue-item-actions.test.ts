import { describe, expect, test } from 'bun:test';
import { readFile } from 'node:fs/promises';

const componentRoot = new URL('../src/components/download/', import.meta.url);

describe('queue item actions', () => {
  for (const component of ['QueueItem.tsx', 'UniversalQueueItem.tsx']) {
    test(`${component} exposes logs for every failed state and copy URL for every inactive state`, async () => {
      const source = await readFile(new URL(component, componentRoot), 'utf8');

      expect(source).toMatch(/\{isError && \(\r?\n\s*<FailedLogsButton/);
      expect(source).not.toContain('isError && !isUpcomingLiveError');
      expect(source).toContain('onClick={handleCopyUrl}');
      expect(source).toContain("t('queue.copyUrl')");
    });
  }

  test('GalleryQueueItem exposes logs on failure and copy URL whenever inactive', async () => {
    const source = await readFile(new URL('GalleryQueueItem.tsx', componentRoot), 'utf8');

    expect(source).toContain('<FailedLogsButton');
    expect(source).toContain('{!isActive && (');
    expect(source).toContain('onClick={handleCopyUrl}');
    expect(source).toContain("tDownload('queue.copyUrl')");
  });
});
