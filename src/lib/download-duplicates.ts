import { normalizeUniversalUrl } from '@/lib/sources';
import type {
  DownloadDuplicateCandidate,
  DownloadDuplicateIdentity,
  DownloadDuplicateMatch,
} from '@/lib/types';
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

function getDownloadDuplicateIdentityKeys(identity: DownloadDuplicateIdentity): string[] {
  return [
    getDownloadDuplicateIdentityKey({ mediaId: identity.mediaId }),
    getDownloadDuplicateIdentityKey({ canonicalUrl: identity.canonicalUrl }),
  ].filter(Boolean);
}

export interface DownloadDuplicateResolution<T extends DownloadDuplicateCandidate> {
  available: T[];
  existing: Array<{ candidate: T; duplicate: DownloadDuplicateMatch }>;
}

export function resolveDownloadedDuplicateCandidates<T extends DownloadDuplicateCandidate>(
  candidates: T[],
  matches: DownloadDuplicateMatch[],
): DownloadDuplicateResolution<T> {
  const matchByKey = new Map<string, DownloadDuplicateMatch>();
  for (const match of matches) {
    for (const key of getDownloadDuplicateIdentityKeys(match)) {
      matchByKey.set(key, match);
    }
  }

  const available: T[] = [];
  const existing: Array<{ candidate: T; duplicate: DownloadDuplicateMatch }> = [];

  for (const candidate of candidates) {
    const duplicate = getDownloadDuplicateIdentityKeys(candidate.duplicateIdentity)
      .map((key) => matchByKey.get(key))
      .find(Boolean);
    if (!duplicate) {
      available.push(candidate);
    } else if (duplicate.fileExists) {
      existing.push({ candidate, duplicate });
    } else {
      available.push({
        ...candidate,
        historyId: duplicate.historyId,
        outputCollisionPolicy: 'overwrite',
      });
    }
  }

  return { available, existing };
}

export function isActiveDownloadQueueItem(item: { status?: string }): boolean {
  return item.status !== 'completed' && item.status !== 'skipped';
}

export function markInactiveQueueDuplicatesUnique<T extends DownloadDuplicateCandidate>(
  candidates: T[],
  items: Array<{ url: string; status?: string }>,
): T[] {
  const inactiveIdentityKeys = new Set(
    items
      .filter((item) => !isActiveDownloadQueueItem(item))
      .flatMap((item) =>
        getDownloadDuplicateIdentityKeys(buildDownloadDuplicateIdentity(item.url)),
      ),
  );

  if (inactiveIdentityKeys.size === 0) return candidates;

  return candidates.map((candidate) =>
    getDownloadDuplicateIdentityKeys(candidate.duplicateIdentity).some((key) =>
      inactiveIdentityKeys.has(key),
    )
      ? { ...candidate, outputCollisionPolicy: 'unique' as const }
      : candidate,
  );
}

export function partitionDownloadQueueUrls<T extends { id: string; url: string; status?: string }>(
  urls: string[],
  items: T[],
): { alreadyQueuedItems: T[]; newUrls: string[] } {
  const itemByIdentity = new Map(
    items
      .filter(isActiveDownloadQueueItem)
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
