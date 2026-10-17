import { afterEach, expect, test } from 'vitest';
import type { ApiError } from '@sungeki/api-client';
import { locale } from '../../shared/i18n/locale.svelte';
import { collectionError } from './presentation';

afterEach(() => { locale.current = 'ja'; });
const error: ApiError = { code: 'IO_ERROR', message: 'original detail', retryable: false,
  suggested_actions: [], details: { reason: 'insufficient_space', os_error: '28' } };
test('capacity guidance is localized without changing durable error details', () => {
  const before = structuredClone(error);
  expect(collectionError(error)).toContain('空き容量');
  expect(collectionError(error)).toContain('事前確認');
  locale.current = 'en';
  expect(collectionError(error)).toContain('choose another destination');
  expect(collectionError(error)).toContain('Check without copying');
  expect(error).toEqual(before);
});
test('other IO and uncertain publication keep their original explanation', () => {
  expect(collectionError({ ...error, details: {} })).toBe(error.message);
  expect(collectionError({ ...error, details: { publication: 'durability_uncertain' } })).toBe(error.message);
  expect(collectionError({ ...error, code: 'VALIDATION_FAILED' })).toBe(error.message);
});
