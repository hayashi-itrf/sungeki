import type { RenderJob, SessionSnapshot } from '@sungeki/api-client';
export function playbackMatches(job: RenderJob | null, snapshot: SessionSnapshot | null,
  blocked: boolean): job is RenderJob {
  return !blocked && !!snapshot && !!job && job.session_id === snapshot.session_id
    && job.source_revision === snapshot.revision && !job.stale
    && job.phase === 'succeeded' && !!job.output;
}
// Recover a live preparation first; a ready movie still needs exact revision matching.
export function recoverPlayback(jobs: readonly RenderJob[], snapshot: SessionSnapshot | null): RenderJob | null {
  if (!snapshot) return null;
  const own = jobs.filter(job => job.session_id === snapshot.session_id);
  return own.find(job => ['queued', 'running', 'cancelling'].includes(job.phase))
    ?? own.find(job => playbackMatches(job, snapshot, false)) ?? null;
}
// Media owns the running clock; editor changes are explicit seek requests.
export class PlaybackClock {
  private published: number | null = null;
  private pending: number | null = null;
  seek(target: number, duration: number): number | null {
    if (!Number.isFinite(target) || !Number.isFinite(duration) || duration <= 0) return null;
    if (this.pending === null && this.published !== null && Math.abs(target - this.published) < 1e-6) return null;
    const value = Math.max(0, Math.min(target, duration));
    if (this.pending === value) return null;
    this.pending = value;
    return value;
  }
  sample(time: number, seeking: boolean): number | null {
    if (seeking || this.pending !== null || !Number.isFinite(time) || time < 0) return null;
    this.published = time;
    return time;
  }
  seeked(time: number): number | null {
    this.pending = null;
    return this.sample(time, false);
  }
}
