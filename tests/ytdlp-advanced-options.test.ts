import { describe, expect, it } from 'bun:test';
import {
  getYtdlpAdvancedOptionDefinition,
  sanitizeYtdlpAdvancedOptions,
  YTDLP_ADVANCED_OPTION_DEFINITIONS,
} from '../src/lib/ytdlp-advanced-options';

describe('yt-dlp advanced options catalog', () => {
  it('exposes unique option ids and flags', () => {
    const ids = YTDLP_ADVANCED_OPTION_DEFINITIONS.map((definition) => definition.id);
    expect(new Set(ids).size).toBe(ids.length);
  });

  it('declares conflicting options symmetrically so the UI cannot offer a one-way conflict', () => {
    for (const definition of YTDLP_ADVANCED_OPTION_DEFINITIONS) {
      for (const conflictId of definition.conflictsWith ?? []) {
        const other = getYtdlpAdvancedOptionDefinition(conflictId);
        expect(other).toBeDefined();
        expect(other?.conflictsWith ?? []).toContain(definition.id);
      }
    }
  });

  it('gives every select option a non-empty choice list', () => {
    for (const definition of YTDLP_ADVANCED_OPTION_DEFINITIONS) {
      if (definition.valueType === 'select') {
        expect(definition.options?.length ?? 0).toBeGreaterThan(0);
      }
    }
  });
});

describe('sanitizeYtdlpAdvancedOptions', () => {
  it('returns an empty list for non-array input', () => {
    for (const input of [undefined, null, 'x', 42, {}]) {
      expect(sanitizeYtdlpAdvancedOptions(input)).toEqual([]);
    }
  });

  it('drops unknown ids so stale persisted settings cannot reach the yt-dlp CLI', () => {
    const sanitized = sanitizeYtdlpAdvancedOptions([
      { id: 'impersonate', value: 'chrome' },
      { id: 'definitelyNotAnOption', value: '--exec rm -rf /' },
      { id: '--exec', value: 'payload' },
    ]);

    expect(sanitized).toEqual([{ id: 'impersonate', value: 'chrome' }]);
  });

  it('keeps only string values and preserves header secondary values', () => {
    const sanitized = sanitizeYtdlpAdvancedOptions([
      { id: 'socketTimeout', value: 30 },
      { id: 'addHeaders', value: 'X-Trace', secondaryValue: 'abc' },
      { id: 'geoBypass' },
    ]);

    expect(sanitized).toEqual([
      { id: 'socketTimeout' },
      { id: 'addHeaders', value: 'X-Trace', secondaryValue: 'abc' },
      { id: 'geoBypass' },
    ]);
  });

  it('ignores malformed entries instead of throwing', () => {
    expect(sanitizeYtdlpAdvancedOptions([null, undefined, 'impersonate', 7, []])).toEqual([]);
  });
});
