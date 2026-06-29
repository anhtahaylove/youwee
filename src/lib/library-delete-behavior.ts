export const LIBRARY_DELETE_FILE_BEHAVIOR_KEY = 'youwee_library_delete_file_behavior';

export const LIBRARY_DELETE_FILE_BEHAVIORS = ['ask', 'delete-file', 'keep-file'] as const;

export type LibraryDeleteFileBehavior = (typeof LIBRARY_DELETE_FILE_BEHAVIORS)[number];

export function normalizeLibraryDeleteFileBehavior(value: unknown): LibraryDeleteFileBehavior {
  return LIBRARY_DELETE_FILE_BEHAVIORS.includes(value as LibraryDeleteFileBehavior)
    ? (value as LibraryDeleteFileBehavior)
    : 'ask';
}

export function loadLibraryDeleteFileBehavior(): LibraryDeleteFileBehavior {
  if (typeof window === 'undefined') return 'ask';
  return normalizeLibraryDeleteFileBehavior(
    window.localStorage.getItem(LIBRARY_DELETE_FILE_BEHAVIOR_KEY),
  );
}

export function saveLibraryDeleteFileBehavior(behavior: LibraryDeleteFileBehavior): void {
  window.localStorage.setItem(LIBRARY_DELETE_FILE_BEHAVIOR_KEY, behavior);
}
