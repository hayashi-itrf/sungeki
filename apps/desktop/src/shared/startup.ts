import { invoke, isTauri } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

export async function checkNativeShell(onEvent: (message: string) => void): Promise<string> {
  if (!isTauri()) return 'ブラウザ表示：ネイティブ機能と編集ホストは未接続です。';
  let resolveEvent!: (message: string) => void;
  const received = new Promise<string>((resolve) => { resolveEvent = resolve; });
  const unlisten = await listen<string>('next-shell-ready', (event) => resolveEvent(event.payload));
  let timer: ReturnType<typeof setTimeout> | undefined;
  try {
    const timeout = new Promise<never>((_, reject) => {
      timer = setTimeout(() => reject(new Error('ネイティブ起動確認がタイムアウトしました')), 5000);
    });
    const [status, message] = await Promise.race([
      Promise.all([invoke<string>('check_shell'), received]),
      timeout,
    ]);
    onEvent(message);
    return status;
  } finally {
    clearTimeout(timer);
    unlisten();
  }
}
