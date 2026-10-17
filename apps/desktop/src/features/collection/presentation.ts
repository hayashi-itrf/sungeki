import type { ApiError, CollectionJob, SessionSnapshot } from '@sungeki/api-client';
import { collectionText } from './messages';
export function collectionError(error: ApiError): string {
  return error.code === 'IO_ERROR' && error.details.reason === 'insufficient_space'
    ? collectionText('保存先の空き容量が不足しています。空き容量を増やすか別の保存先を選び、「コピーせず事前確認」からやり直してください。')
    : error.message;
}
export function destination(parent: string, name: string): string | null {
  const leaf = name.trim();
  if (!parent.startsWith('/') || !leaf || leaf === '.' || leaf === '..' || /[\\/:\0]/.test(leaf)) return null;
  return `${parent.replace(/\/+$/, '')}/${leaf}`;
}
export function canCollect(job: CollectionJob | null, snapshot: SessionSnapshot | null, path: string | null, includeCheckpoints = false) {
  return !!job && !!snapshot && !!path && job.session_id === snapshot.session_id &&
    job.phase === 'succeeded' && job.include_checkpoints === includeCheckpoints && job.dry_run && job.plan?.sources_ready === true &&
    !job.plan.destination_exists && !job.stale && job.source_revision === snapshot.revision && job.destination === path;
}
export function bytes(value: number) {
  return value < 1024 ? `${value} B` : value < 1024 ** 2 ? `${(value / 1024).toFixed(1)} KB` : `${(value / 1024 ** 2).toFixed(1)} MB`;
}
export const sourceStatus = { verified: '確認済み', missing: '見つからない', changed: '内容が変更された',
  unresolved: '参照先が不明', unreadable: '読み込めない', not_inspected: '未検査' } as const;
