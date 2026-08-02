import { afterEach, describe, expect, test } from 'bun:test';
import { createClientId } from '../src/lib/client-id';

const originalCrypto = Object.getOwnPropertyDescriptor(globalThis, 'crypto');

afterEach(() => {
  if (originalCrypto) {
    Object.defineProperty(globalThis, 'crypto', originalCrypto);
  } else {
    Reflect.deleteProperty(globalThis, 'crypto');
  }
});

describe('createClientId', () => {
  test('uses getRandomValues when randomUUID is unavailable', () => {
    Object.defineProperty(globalThis, 'crypto', {
      configurable: true,
      value: {
        getRandomValues(values: Uint8Array) {
          values.fill(0xab);
          return values;
        },
      },
    });

    expect(createClientId('queue')).toBe(`queue-${'ab'.repeat(16)}`);
  });

  test('falls back without a crypto API', () => {
    Object.defineProperty(globalThis, 'crypto', {
      configurable: true,
      value: undefined,
    });

    expect(createClientId('queue')).toMatch(/^queue-\d+-[a-z0-9]+$/);
  });
});
