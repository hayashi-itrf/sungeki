import { afterEach, expect, test, vi } from 'vitest';
import type { EventBatch, SessionSnapshot } from '@sungeki/api-client';
import { syncSession } from './sync';

const cursor = { host_instance_id: 'h', event_seq: 1 };
const initial = { session_id: 'a', event_cursor: cursor } as SessionSnapshot;
const batch: EventBatch = { cursor: { ...cursor, event_seq: 2 }, resync_required: false,
  events: [{ session_id: 'a', revision: 1, event_seq: 2, type: 'edited', payload: { operation_id: 'op' } }] };
const never = () => new Promise<EventBatch>(() => {});
afterEach(() => vi.useRealTimers());

test('notification fetches full state and stop ignores an in-flight response', async () => {
  let resolve!: (batch: EventBatch) => void;
  const watch = vi.fn().mockReturnValue(new Promise<EventBatch>(r => { resolve = r; }));
  const session = vi.fn(); const receive = vi.fn();
  const stop = syncSession({ watch, session }, initial, receive, vi.fn());
  stop(); resolve(batch);
  await Promise.resolve();
  expect(session).not.toHaveBeenCalled();
  expect(receive).not.toHaveBeenCalled();
});
test('failed refresh retains the cursor, then reconnects and refreshes', async () => {
  vi.useFakeTimers();
  const watch = vi.fn().mockResolvedValueOnce(batch).mockResolvedValueOnce(batch).mockImplementation(never);
  const updated = { ...initial, revision: 1 };
  const session = vi.fn().mockRejectedValueOnce(new Error('disconnected')).mockResolvedValue({ type: 'snapshot', snapshot: updated });
  const receive = vi.fn(); const status = vi.fn();
  const stop = syncSession({ watch, session }, initial, receive, status);
  expect(status).toHaveBeenLastCalledWith('connecting');
  await vi.advanceTimersByTimeAsync(1000);
  expect(status.mock.calls.map(c => c[0])).toEqual(['connecting', 'retrying', 'live']);
  expect(watch.mock.calls[1]?.[0]).toEqual(cursor);
  expect(receive).toHaveBeenCalledWith(updated);
  expect(watch.mock.calls[2]?.[0]).toEqual(batch.cursor);
  stop();
});
test('host restart or missing events force a snapshot even without event entries', async () => {
  const watch = vi.fn().mockResolvedValueOnce({ cursor: { host_instance_id: 'new', event_seq: 0 }, events: [], resync_required: true }).mockImplementation(never);
  const session = vi.fn().mockResolvedValue({ type: 'snapshot', snapshot: initial });
  const receive = vi.fn();
  const stop = syncSession({ watch, session }, initial, receive, vi.fn());
  await vi.waitFor(() => expect(receive).toHaveBeenCalledOnce());
  expect(session).toHaveBeenCalledWith({ type: 'get', session_id: 'a' });
  stop();
});

test('checkpoint notifications and reconnect refresh the checkpoint list', async () => {
  const changed = vi.fn();
  const watch = vi.fn().mockResolvedValueOnce({ ...batch,
    events: [{ session_id: 'a', revision: 0, event_seq: 2, type: 'checkpoint_created', payload: {} }] })
    .mockResolvedValueOnce({ ...batch, events: [{ session_id: 'another-session', revision: 0, event_seq: 3, type: 'checkpoint_deleted', payload: {} }] })
    .mockResolvedValueOnce({ ...batch, resync_required: true, events: [] }).mockImplementation(never);
  const session = vi.fn().mockResolvedValue({ type: 'snapshot', snapshot: initial });
  const stop = syncSession({ watch, session }, initial, vi.fn(), vi.fn(), 1000, changed);
  await vi.waitFor(() => expect(changed).toHaveBeenCalledTimes(3));
  expect(changed.mock.calls.map(c => c[0])).toEqual([false, true, true]);
  stop();
});

test('already observed edit events skip fetch but still advance watch cursor', async () => {
  const watch = vi.fn().mockResolvedValueOnce(batch).mockImplementation(never);
  const session = vi.fn(); const receive = vi.fn();
  const known = { ...initial, revision: 1, event_cursor: batch.cursor };
  const stop = syncSession({ watch, session }, initial, receive, vi.fn(), 1000, vi.fn(), () => known);
  await vi.waitFor(() => expect(watch).toHaveBeenCalledTimes(2));
  expect(session).not.toHaveBeenCalled(); expect(receive).not.toHaveBeenCalled();
  expect(watch.mock.calls[1]?.[0]).toEqual(batch.cursor);
  stop();
});

test('unseen same-revision save, another session/host and resync still fetch', async () => {
  const saved: EventBatch = { ...batch, events: [{ ...batch.events[0], type: 'saved', revision: 1 }] };
  const observed = { ...initial, revision: 1, event_cursor: batch.cursor };
  const cases: [SessionSnapshot | null, EventBatch][] = [
    [{ ...observed, event_cursor: cursor }, saved],
    [{ ...observed, session_id: 'other' }, batch],
    [{ ...observed, event_cursor: { ...batch.cursor, host_instance_id: 'old' } }, batch],
    [observed, { ...batch, resync_required: true, events: [] }],
    [null, batch],
  ];
  for (const [known, notification] of cases) {
    const watch = vi.fn().mockResolvedValueOnce(notification).mockImplementation(never);
    const session = vi.fn().mockResolvedValue({ type: 'snapshot', snapshot: observed });
    const receive = vi.fn();
    const stop = syncSession({ watch, session }, initial, receive, vi.fn(), 1000, vi.fn(), () => known);
    await vi.waitFor(() => expect(receive).toHaveBeenCalledOnce());
    expect(session).toHaveBeenCalledOnce(); stop();
  }
});

test('observed checkpoint event still refreshes checkpoint list without fetching Project', async () => {
  const notification = { ...batch, events: [{ ...batch.events[0], type: 'checkpoint_created' as const }] };
  const watch = vi.fn().mockResolvedValueOnce(notification).mockImplementation(never);
  const session = vi.fn(); const changed = vi.fn();
  const stop = syncSession({ watch, session }, initial, vi.fn(), vi.fn(), 1000, changed,
    () => ({ ...initial, event_cursor: batch.cursor }));
  await vi.waitFor(() => expect(changed).toHaveBeenCalledWith(false));
  expect(session).not.toHaveBeenCalled(); stop();
});
