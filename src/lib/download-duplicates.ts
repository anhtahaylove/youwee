import type { DownloadDuplicateIdentity } from '@/lib/types';
import { extractYouTubeVideoId } from '@/lib/youtube-url';

function stripUrlFragment(url: string): string {
  try {
    const parsed = new URL(url.trim());
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
