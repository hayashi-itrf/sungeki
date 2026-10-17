import { afterEach, expect, test, vi } from 'vitest';
import { invoke, isTauri } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { checkNativeShell } from './startup';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn(), isTauri: vi.fn() }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn() }));
afterEach(() => vi.resetAllMocks());

test('ブラウザではネイティブ接続成功と表示しない', async () => {
  vi.mocked(isTauri).mockReturnValue(false);
  expect(await checkNativeShell(vi.fn())).toContain('未接続');
  expect(invoke).not.toHaveBeenCalled();
  expect(listen).not.toHaveBeenCalled();
});

test('ネイティブ要求失敗でもイベント購読を解放する', async () => {
  vi.mocked(isTauri).mockReturnValue(true);
  const unlisten = vi.fn();
  vi.mocked(listen).mockResolvedValue(unlisten);
  vi.mocked(invoke).mockRejectedValue(new Error('切断'));
  await expect(checkNativeShell(vi.fn())).rejects.toThrow('切断');
  expect(unlisten).toHaveBeenCalledOnce();
});

test('応答より遅いイベントも受信してから成功する', async () => {
  vi.mocked(isTauri).mockReturnValue(true);
  const unlisten = vi.fn();
  let deliver!: () => void;
  vi.mocked(listen).mockImplementation(async (_, handler) => {
    deliver = () => handler({ event: 'next-shell-ready', id: 1, payload: '受信' } as never);
    return unlisten;
  });
  vi.mocked(invoke).mockResolvedValue('成功');
  const callback = vi.fn();
  const result = checkNativeShell(callback);
  await Promise.resolve();
  await Promise.resolve();
  expect(unlisten).not.toHaveBeenCalled();
  deliver();
  expect(await result).toBe('成功');
  expect(callback).toHaveBeenCalledWith('受信');
  expect(unlisten).toHaveBeenCalledOnce();
});

test('イベントが来ない場合は有限待機で購読を解放する', async () => {
  vi.useFakeTimers();
  try {
    vi.mocked(isTauri).mockReturnValue(true);
    const unlisten = vi.fn();
    vi.mocked(listen).mockResolvedValue(unlisten);
    vi.mocked(invoke).mockResolvedValue('成功');
    const result = checkNativeShell(vi.fn());
    const assertion = expect(result).rejects.toThrow('タイムアウト');
    await vi.advanceTimersByTimeAsync(5000);
    await assertion;
    expect(unlisten).toHaveBeenCalledOnce();
  } finally {
    vi.useRealTimers();
  }
});
