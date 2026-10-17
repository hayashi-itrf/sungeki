import { invoke, isTauri } from '@tauri-apps/api/core';
import { HostApi } from '@sungeki/api-client';
import type { ApiError, HostInfo, SessionRequest, SessionResponse, EventCursor, EventBatch } from '@sungeki/api-client';

function requireNative() {
  if (!isTauri()) {
    const error: ApiError = { code: 'HOST_UNAVAILABLE', message: '編集にはSungekiアプリを起動してください。ブラウザは未接続です。', details: {}, retryable: false, suggested_actions: [] };
    throw error;
  }
}
export const hostApi = new HostApi({
  async watch(after: EventCursor | null, waitMs: number): Promise<EventBatch> {
    requireNative();
    return invoke<EventBatch>('watch_sessions', { after, waitMs });
  },
  async session(request: SessionRequest): Promise<SessionResponse> {
    requireNative();
    return invoke<SessionResponse>('session_request', { request });
  },
  async getCapabilities(): Promise<HostInfo> {
    if (!isTauri()) {
      const error: ApiError = {
        code: 'HOST_UNAVAILABLE', message: 'ブラウザ表示では共通ホストに接続できません。Sungekiアプリを起動してください。',
        details: {}, retryable: false, suggested_actions: [],
      };
      throw error;
    }
    return invoke<HostInfo>('get_host_capabilities');
  },
});
