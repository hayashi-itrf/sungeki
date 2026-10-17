import { afterEach, expect, test } from 'vitest';
import { locale } from '../../shared/i18n/locale.svelte';
import { runtimeDiagnostic } from './runtime-diagnostic';

afterEach(() => { locale.current = 'ja'; });

test('storage guidance follows the current language while preserving paths and OS details', () => {
  const details = '/Volumes/動画 SSD/$&/missing: No such file or directory (os error 2)';
  const message = '保存フォルダを確認できません。外部ディスク内の既存フォルダを選び直してください: ' + details;
  expect(runtimeDiagnostic(message)).toBe(message);
  locale.current = 'en';
  expect(runtimeDiagnostic(message)).toBe('Cannot access the storage folder. Choose an existing folder on your external disk: ' + details);
  locale.current = 'ja';
  expect(runtimeDiagnostic(message)).toBe(message);
});

test('unknown errors and similar user-supplied text remain unchanged', () => {
  locale.current = 'en';
  for (const message of ['', 'Permission denied (os error 13)', '未知の診断',
    '/Volumes/保存フォルダを確認できません。/file',
    '保存フォルダを確認できません。追加の診断']) {
    expect(runtimeDiagnostic(message)).toBe(message);
  }
});
