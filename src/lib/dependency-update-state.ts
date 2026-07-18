export type ManagedDependencyUpdateState =
  | 'update-available'
  | 'up-to-date'
  | 'packaged'
  | 'system'
  | 'installed';

export function resolveManagedDependencyUpdateState({
  updateInfo,
  isSystem,
  isBundled,
}: {
  updateInfo: { has_update: boolean } | null | undefined;
  isSystem: boolean;
  isBundled: boolean;
}): ManagedDependencyUpdateState {
  if (isSystem) return 'system';
  if (updateInfo) return updateInfo.has_update ? 'update-available' : 'up-to-date';
  if (isBundled) return 'packaged';
  return 'installed';
}
