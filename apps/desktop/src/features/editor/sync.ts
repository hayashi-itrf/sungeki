import type { EventCursor, HostApi, SessionSnapshot } from '@sungeki/api-client';

export type SyncStatus = 'connecting' | 'live' | 'resynced' | 'retrying';

type SyncApi = Pick<HostApi, 'watch' | 'session'>;
export function syncSession(api: SyncApi, initial: SessionSnapshot,
  receive: (snapshot: SessionSnapshot) => void, status: (state: SyncStatus) => void,
  retryMs = 1000, checkpointChanged: (reset: boolean) => void = () => {},
  currentSnapshot: () => SessionSnapshot | null = () => null): () => void {
  let stopped = false;
  let cursor: EventCursor | null = initial.event_cursor;
  let retry: ReturnType<typeof setTimeout> | undefined;
  let releaseRetry: (() => void) | undefined;
  async function run() {
    while (!stopped) {
      try {
        const batch = await api.watch(cursor, 20_000);
        if (stopped) break;
        const known = currentSnapshot();
        const observed = known?.session_id === initial.session_id
          && known.event_cursor.host_instance_id === batch.cursor.host_instance_id
          ? known.event_cursor.event_seq : -1;
        // An edit/save response may already include these events. Do not skip
        // unseen events (including saves at the same revision) or forced resync.
        if (batch.resync_required || batch.events.some(e => e.session_id === initial.session_id && e.event_seq > observed)) {
          const response = await api.session({ type: 'get', session_id: initial.session_id });
          if (stopped) break;
          if (response.type !== 'snapshot' && response.type !== 'edited') throw new Error('文書の応答形式が不正です');
          receive(response.snapshot);
        }
        // Checkpoints belong to a Project, which may have more than one open session.
        const removed = batch.events.some(e => e.type === 'checkpoint_deleted');
        if (batch.resync_required || removed || batch.events.some(e => e.type === 'checkpoint_created')) checkpointChanged(batch.resync_required || removed);
        // Advance only after snapshot retrieval succeeds; failed refresh cannot lose a change.
        cursor = batch.cursor;
        status(batch.resync_required ? 'resynced' : 'live');
      } catch (error) {
        if (stopped) break;
        status('retrying');
        await new Promise<void>(resolve => {
          releaseRetry = resolve;
          retry = setTimeout(resolve, retryMs);
        });
        releaseRetry = undefined;
      }
    }
  }
  status('connecting');
  void run();
  return () => { stopped = true; clearTimeout(retry); releaseRetry?.(); };
}
