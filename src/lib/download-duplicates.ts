import { normalizeUniversalUrl } from '@/lib/sources';
import type { DownloadDuplicateIdentity } from '@/lib/types';
import { extractYouTubeVideoId } from '@/lib/youtube-url';

function stripUrlFragment(url: string): string {
  try {
    const parsed = new URL(normalizeUniversalUrl(url));
    parsed.hash = '';
    return parsed.toString();
  } catch {
    return url.trim();
  }
}

export function buildDownloadDuplicateIdentity(
  url: string,
  youtubeVideoId?: string | null,
): DownloadDuplicateIdentity {
  const videoId = youtubeVideoId || extractYouTubeVideoId(url);

  if (videoId) {
    return {
      mediaId: `youtube:${videoId}`,
      canonicalUrl: `https://www.youtube.com/watch?v=${videoId}`,
    };
  }

  return {
    mediaId: null,
    canonicalUrl: stripUrlFragment(url),
  };
}

export function getDownloadDuplicateIdentityKey(identity: DownloadDuplicateIdentity): string {
  const mediaId = identity.mediaId?.trim();
  if (mediaId) return `media:${mediaId}`;

  const canonicalUrl = identity.canonicalUrl?.trim();
  if (canonicalUrl) return `url:${canonicalUrl}`;

  return '';
}

export function partitionDownloadQueueUrls<T extends { id: string; url: string }>(
  urls: string[],
  items: T[],
): { alreadyQueuedItems: T[]; newUrls: string[] } {
  const itemByIdentity = new Map(
    items
      .map(
        (item) =>
          [
            getDownloadDuplicateIdentityKey(buildDownloadDuplicateIdentity(item.url)),
            item,
          ] as const,
      )
      .filter(([identity]) => Boolean(identity)),
  );
  const alreadyQueuedItems = Array.from(
    new Map(
      urls
        .map((url) =>
          itemByIdentity.get(getDownloadDuplicateIdentityKey(buildDownloadDuplicateIdentity(url))),
        )
        .filter((item): item is T => Boolean(item))
        .map((item) => [item.id, item]),
    ).values(),
  );
  const newUrls = urls.filter(
    (url) =>
      !itemByIdentity.has(getDownloadDuplicateIdentityKey(buildDownloadDuplicateIdentity(url))),
  );

  return { alreadyQueuedItems, newUrls };
}
