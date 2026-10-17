import { afterEach, expect, test, vi } from 'vitest';
import type { SessionSnapshot } from '@sungeki/api-client';
import { locale } from '../../shared/i18n/locale.svelte';
import { ReviewController } from './controller.svelte';
import { reviewDiagnostic } from './review-diagnostic';

afterEach(() => { locale.current = 'ja'; });
test('start rejection displays recovery actions and clears them on the next failure', async () => {
  const session = vi.fn().mockRejectedValueOnce({ code: 'CAPABILITY_UNAVAILABLE', message: '保存先を確認',
    suggested_actions: ['保存フォルダを設定', 42, '再度開始'] })
    .mockRejectedValueOnce({ code: 'HOST_UNAVAILABLE', message: '通信切断' });
  const controller = new ReviewController({ session }, 'session');
  const snapshot = { session_id: 'session', revision: 2 } as SessionSnapshot;
  await controller.start(snapshot);
  expect(controller.error).toBe('保存先を確認\n保存フォルダを設定\n再度開始');
  expect(controller.pending).toBeNull();
  await controller.start(snapshot);
  expect(controller.error).toBe('通信切断');
});
test('English setup advice preserves unknown diagnostics', () => {
  locale.current = 'en';
  expect(reviewDiagnostic({message:'固有の診断', suggested_actions:[
    '「設定」→「動画制作の実行環境」で外部ディスクの保存フォルダを選んで保存する'
  ]})).toBe('固有の診断\nIn Settings → Video production tools, choose a storage folder on your external disk and save the settings.');
  expect(reviewDiagnostic(null)).toBe('null');
});
