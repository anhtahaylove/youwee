import { describe, expect, test } from 'bun:test';
import {
  DEFAULT_FILENAME_METADATA_FIELDS,
  resolveFilenameMetadataSettings,
  sanitizeFilenameMetadataFields,
} from '../src/lib/filename-metadata';

describe('filename metadata settings', () => {
  test('defaults the feature off without trusting unknown values', () => {
    expect(resolveFilenameMetadataSettings(undefined, undefined)).toEqual({
      enabled: false,
      fields: [],
    });
  });

  test('keeps allowlisted fields in user order without duplicates', () => {
    expect(
      sanitizeFilenameMetadataFields(['videoId', '--output', 'uploadDate', 'videoId', null]),
    ).toEqual(['videoId', 'uploadDate']);
  });

  test('provides conservative defaults when the user enables the feature', () => {
    expect(DEFAULT_FILENAME_METADATA_FIELDS).toEqual(['uploadDate', 'viewCount']);
  });
});
