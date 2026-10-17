import type { ProxyJob, SessionSnapshot } from '@sungeki/api-client';

/** View-only choices: never serialized into Project or editor history. */
export class ProxySelection {
  private sessionId = $state('');
  private choices = $state<Record<string, string>>({});

  forSession(sessionId: string): Readonly<Record<string, string>> {
    return sessionId === this.sessionId ? this.choices : {};
  }
  use(snapshot: SessionSnapshot, job: ProxyJob, assetId: string): boolean {
    if (job.session_id !== snapshot.session_id || !job.completed.some(o => o.asset_id === assetId)
      || !job.asset_ids.includes(assetId)
      || !snapshot.project.assets.some(a => a.id === assetId && a.probe.type === 'video')) return false;
    if (this.sessionId !== snapshot.session_id) this.choices = {};
    this.sessionId = snapshot.session_id;
    this.choices = { ...this.choices, [assetId]: job.job_id };
    return true;
  }
  original(sessionId: string, assetId: string) {
    if (this.sessionId !== sessionId) return;
    const next = { ...this.choices };
    delete next[assetId]; this.choices = next;
  }
  clear() { this.sessionId = ''; this.choices = {}; }
}
