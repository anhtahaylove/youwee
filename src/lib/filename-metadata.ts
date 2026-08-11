import type { FilenameMetadataField } from '@/lib/types';

export const DEFAULT_FILENAME_METADATA_FIELDS: FilenameMetadataField[] = [
  'uploadDate',
  'viewCount',
];

export const FILENAME_METADATA_FIELDS: FilenameMetadataField[] = [
  'uploadDate',
  'viewCount',
  'uploader',
  'duration',
  'resolution',
  'videoId',
];

const FILENAME_METADATA_FIELD_SET = new Set<FilenameMetadataField>(FILENAME_METADATA_FIELDS);

export function sanitizeFilenameMetadataFields(value: unknown): FilenameMetadataField[] {
  if (!Array.isArray(value)) return [];

  const fields: FilenameMetadataField[] = [];
  for (const field of value) {
    if (
      typeof field === 'string' &&
      FILENAME_METADATA_FIELD_SET.has(field as FilenameMetadataField) &&
      !fields.includes(field as FilenameMetadataField)
    ) {
      fields.push(field as FilenameMetadataField);
    }
  }
  return fields;
}

export function resolveFilenameMetadataSettings(
  enabled: unknown,
  fields: unknown,
): { enabled: boolean; fields: FilenameMetadataField[] } {
  return {
    enabled: enabled === true,
    fields: sanitizeFilenameMetadataFields(fields),
  };
}
