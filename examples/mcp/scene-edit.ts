import { randomUUID } from 'node:crypto';
import type { ApplyCommandsArgs, EditPreview, HistoryArgs, SessionResponse } from '../../packages/api-client/src/generated.ts';
import type { McpClient } from './client.ts';

type Api = Pick<McpClient, 'call'>;
export type SceneMovePlan = { request: ApplyCommandsArgs; previews: EditPreview[] };
type Edited = Extract<SessionResponse, { type: 'edited' }>;

/** Read all differences at one revision. Nothing is applied while planning. */
export async function planSceneMove(api: Api, sessionId: string, sceneId: string, startUs: number): Promise<SceneMovePlan> {
  if (!Number.isSafeInteger(startUs) || startUs < 0) throw new Error('startUs must be a non-negative safe integer');
  const read = await api.call('get_project', { session_id: sessionId, view: 'summary' }) as SessionResponse;
  if (read.type !== 'project_read') throw new Error('Unexpected project summary');
  const request: ApplyCommandsArgs = {
    session_id: sessionId, expected_revision: read.result.revision, operation_id: randomUUID(),
    commands: [{ type: 'move_scene', scene_id: sceneId, start_us: startUs }],
    dry_run: false, offset: null, limit: null,
  };
  const previews: EditPreview[] = [];
  let offset: number | null = 0;
  do {
    const result = await api.call('apply_commands', { ...request, dry_run: true, offset, limit: 50 }) as SessionResponse;
    if (result.type !== 'edit_preview') throw new Error('Unexpected edit preview');
    if (result.preview.source_revision !== request.expected_revision) throw new Error('Preview revision changed');
    const next = result.preview.next_offset;
    if (next !== null && next <= offset) throw new Error('Invalid preview pagination');
    previews.push(result.preview); offset = next;
  } while (offset !== null);
  return { request, previews };
}

/** Keep this exact request/operation ID for retries; do not refresh its revision. */
export async function applySceneMove(api: Api, plan: SceneMovePlan): Promise<Edited> {
  const result = await api.call('apply_commands', plan.request) as SessionResponse;
  if (result.type !== 'edited') throw new Error('Unexpected edit result');
  return result;
}

/** Store the returned request before sending. Undo targets this edit, not someone else's. */
export function sceneMoveUndo(applied: Edited): HistoryArgs {
  if (applied.receipt.no_op) throw new Error('A no-op has no history entry to undo');
  return {
    session_id: applied.snapshot.session_id, expected_revision: applied.receipt.revision,
    operation_id: randomUUID(), expected_operation_id: applied.receipt.operation_id,
  };
}
