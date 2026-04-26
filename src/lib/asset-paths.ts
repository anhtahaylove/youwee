export function normalizeAssetPath(path: string): string {
  return path.trim().replace(/^\\\\\?\\/, '');
}

export function collectAssetScopeCandidates(paths: string[]): string[] {
  const seen = new Set<string>();
  const candidates: string[] = [];

  for (const path of paths) {
    const normalized = normalizeAssetPath(path);
    if (!normalized || seen.has(normalized)) continue;
    seen.add(normalized);
    candidates.push(normalized);
  }

  return candidates;
}
