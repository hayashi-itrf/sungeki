import type { SessionSnapshot } from '@sungeki/api-client';

/** Reuse immutable Project data when only host/save metadata has changed. */
export function reuseSnapshot(current: SessionSnapshot | null, next: SessionSnapshot): SessionSnapshot {
  // Revision identifies a confirmed Project only within the same live host/session.
  if (!current || current.session_id !== next.session_id || current.revision !== next.revision
    || current.event_cursor.host_instance_id !== next.event_cursor.host_instance_id
    || current.project.id !== next.project.id) return next;
  if (current.event_cursor.event_seq === next.event_cursor.event_seq
    && current.dirty === next.dirty && current.document_path === next.document_path
    && current.undo_operation_id === next.undo_operation_id
    && current.redo_operation_id === next.redo_operation_id) return current;
  return { ...next, project: current.project };
}
