import { readPreviewVolume, savePreviewVolume } from '../../shared/preview-volume';
import { locale } from '../../shared/i18n/locale.svelte';
import { editErrorMessage } from '../../shared/edit-error';
import type { SyncStatus } from './sync';
import { uiPerformance } from '../../shared/ui-performance.svelte';
import { reuseSnapshot } from './snapshot-reuse';
import { contrastingTextColor } from '../../shared/text-contrast';
import type { Command, HostApi, SessionRequest, SessionSnapshot } from '@sungeki/api-client';

export class EditorController {
  // Confirmed documents are replaced by host responses, never edited in place.
  // Avoid creating thousands of deep reactive proxies when reading a large project.
  snapshot = $state.raw<SessionSnapshot | null>(null);
  sessions = $state<string[]>([]);
  busy = $state(false);
  pending = $state.raw<SessionRequest | null>(null);
  error = $state('');
  syncState = $state<SyncStatus>('connecting');
  get syncStatus() {
    const keys = { connecting: 'editorSyncConnecting', live: 'editorSyncLive',
      resynced: 'editorSyncResynced', retrying: 'editorSyncRetrying' } as const;
    return locale.text(keys[this.syncState]);
  }
  drafting = $state(false);
  draftGeneration = $state(0);
  checkpointGeneration = $state(0);
  checkpointResetGeneration = $state(0);
  incoming = $state.raw<SessionSnapshot | null>(null);
  private primary = $state<string | null>(null);
  selection = $state<string[]>([]);
  dragPreview = $state.raw<{ ids: ReadonlySet<string>; delta: number; revision: number } | null>(null);
  focusSelected(id: string) { if (this.selection.includes(id)) this.primary = id; else this.selected = id; }
  get selected() { return this.primary; }
  set selected(id: string | null) { this.primary = id; this.selection = id ? [id] : []; }
  selectClip(id: string, additive = false) {
    if (this.busy || this.pending || this.drafting) return;
    if (!this.snapshot?.project.clips.some(c => c.id === id)) return;
    if (!additive) { this.selected = id; return; }
    this.selection = this.selection.includes(id) ? this.selection.filter(value => value !== id) : [...this.selection, id];
    this.primary = this.selection.includes(id) ? id : this.selection.at(-1) ?? null;
  }
  private reconcileSelection() {
    const ids = new Set(this.snapshot?.project.clips.map(c => c.id));
    this.selection = this.selection.filter(id => ids.has(id));
    if (!this.primary || !ids.has(this.primary)) this.primary = this.selection.at(-1) ?? null;
  }
  // Audition volume belongs to this window, not a document or player instance.
  private auditionVolume = $state(readPreviewVolume());
  get previewVolume() { return this.auditionVolume; }
  set previewVolume(value: number) {
    if (!Number.isFinite(value) || value < 0 || value > 1) return;
    this.auditionVolume = value; savePreviewVolume(value);
  }
  time = $state(0);
  navigationGeneration = $state(0);
  seekGeneration = $state(0);
  revealScene(id: string) {
    if (this.busy || this.pending || this.drafting) return false;
    const scene = this.snapshot?.project.scenes.find(scene => scene.id === id);
    if (!scene) return false;
    this.time = scene.start_us / 1_000_000;
    this.seekGeneration++;
    return true;
  }
  revealClip(id: string, timeUs?: number) {
    if (this.busy || this.pending || this.drafting) return false;
    const clip = this.snapshot?.project.clips.find(clip => clip.id === id);
    if (!clip) return false;
    if (timeUs !== undefined && (!Number.isSafeInteger(timeUs) || timeUs < clip.start_us || timeUs >= clip.start_us + clip.duration_us)) return false;
    const fps = this.snapshot!.project.canvas.fps;
    // Diagnostic times are integer microseconds rounded down from frame time.
    const time = timeUs === undefined ? clip.start_us / 1_000_000
      : Math.ceil(timeUs * fps.numerator / (1_000_000 * fps.denominator)) * fps.denominator / fps.numerator;
    if (timeUs !== undefined && Math.floor(time * 1_000_000) >= clip.start_us + clip.duration_us) return false;
    this.selected = id;
    this.time = time;
    this.navigationGeneration++;
    return true;
  }
  constructor(private api: Pick<HostApi, 'session'>) {}

  async request(request: SessionRequest) {
    if (this.busy) return;
    if (this.pending && request !== this.pending) {
      this.error = locale.text('editorPendingOperation');
      return;
    }
    const finishMeasurement = request.type === 'edit' ? uiPerformance.beginEdit() : null;
    let editSucceeded = false;
    this.busy = true;
    this.error = '';
    try {
      const result = await this.api.session(request);
      editSucceeded = result.type === 'edited';
      this.pending = null;
      if (result.type === 'sessions') this.sessions = result.session_ids;
      else if (result.type !== 'snapshot' && result.type !== 'edited') return;
      // Reading the current document must not discard an unsubmitted form.
      // Remote edits use the same explicit draft-discard flow as live events.
      else if (this.drafting && (request.type === 'save'
        || (request.type === 'get' && result.snapshot.session_id === this.snapshot?.session_id))) this.receive(result.snapshot);
      else {
        const switched = this.snapshot?.session_id !== result.snapshot.session_id;
        this.snapshot = reuseSnapshot(this.snapshot, result.snapshot);
        if (this.drafting) this.draftGeneration++;
        this.drafting = false;
        if (switched) { this.selected = null; this.time = 0; }
        this.reconcileSelection();
      }
    } catch (error) {
      const code = error && typeof error === 'object' && 'code' in error ? error.code : undefined;
      if ((request.type === 'edit' || request.type === 'instantiate_preset' || (request.type === 'checkpoint' && request.request.type === 'restore')) && (code === 'HOST_UNAVAILABLE' || code === 'IO_ERROR' || !code)) this.pending = request;
      else this.pending = null;
      this.error = editErrorMessage(error);
      if (error && typeof error === 'object' && 'code' in error && error.code === 'CONFLICT') {
        this.error += ` ${locale.text('editorConflictHint')}`;
      }
    } finally {
      this.busy = false;
      if (!this.drafting && !this.pending) {
        const error = this.error; this.acceptIncoming(); this.error = error;
      }
      finishMeasurement?.(editSucceeded);
    }
  }
  receive(snapshot: SessionSnapshot) {
    const current = this.snapshot;
    if (!current || current.session_id !== snapshot.session_id || snapshot.revision < current.revision) return;
    if (current.event_cursor.host_instance_id === snapshot.event_cursor.host_instance_id
      && snapshot.event_cursor.event_seq < current.event_cursor.event_seq) return;
    // The visible snapshot stays frozen during input. Compare against buffered
    // updates too, or a late response can replace a newer remote edit/save.
    const queued = this.incoming;
    if (queued?.session_id === snapshot.session_id && (snapshot.revision < queued.revision
      || (queued.event_cursor.host_instance_id === snapshot.event_cursor.host_instance_id
        && snapshot.event_cursor.event_seq < queued.event_cursor.event_seq))) return;
    snapshot = reuseSnapshot(current, snapshot);
    if (snapshot === current) return;
    // Saving changes metadata, not Project/revision; keep the mounted draft intact.
    if (this.drafting && snapshot.revision === current.revision) { this.snapshot = snapshot; return; }
    if (this.busy || this.drafting || this.pending) this.incoming = snapshot;
    else { this.snapshot = snapshot; this.reconcileSelection(); }
  }
  acceptIncoming() {
    const next = this.incoming;
    this.incoming = null;
    if (next) this.error = '';
    if (this.drafting) this.draftGeneration++;
    this.drafting = false;
    if (next) this.receive(next);
  }
  async apply(commands: Command[]) {
    const current = this.snapshot;
    if (!current) return;
    await this.request({ type: 'edit', session_id: current.session_id, edit: {
      operation_id: crypto.randomUUID(), expected_revision: current.revision,
      action: { type: 'apply', commands },
    } });
  }
  async history(type: 'undo' | 'redo') {
    if (this.busy || this.pending || this.drafting) return;
    const current = this.snapshot;
    const target = type === 'undo' ? current?.undo_operation_id : current?.redo_operation_id;
    if (!current || !target) return;
    await this.request({ type: 'edit', session_id: current.session_id, edit: {
      operation_id: crypto.randomUUID(), expected_revision: current.revision,
      action: { type, expected_operation_id: target },
    } });
  }
  async add(kind: 'text' | 'shape' | 'blur') {
    if (!this.snapshot || this.busy || this.pending || this.drafting) return;
    // Each inserted visual is a new front layer; the host validates the atomic batch.
    const trackId = crypto.randomUUID();
    const clipId = crypto.randomUUID();
    const commands: Command[] = [{ type: 'add_track', track: {
      id: trackId, name: kind === 'blur' ? locale.text('clipBlur') : locale.text('editorVisualTrack'), kind: 'visual', visible: true, muted: false,
    } }];
    commands.push({ type: 'add_clip', clip: {
      id: clipId, track_id: trackId, scene_id: null, start_us: Math.round(this.time * 1_000_000), duration_us: 5_000_000,
      transform: kind === 'blur'
        ? { x: 0, y: 0, width: this.snapshot.project.canvas.width, height: this.snapshot.project.canvas.height, opacity: 1 }
        : { x: 120, y: 120, width: 640, height: 160, opacity: 1 },
      content: kind === 'text' ? { type: 'text', text: locale.text('editorNewText'), font_size: 64,
        color: contrastingTextColor(this.snapshot.project.canvas.background) }
        : kind === 'blur' ? { type: 'blur', sigma: 18 } : { type: 'shape', shape: 'rectangle', fill: '#85d6c5' },
    } });
    await this.apply(commands);
    if (!this.error) this.selected = clipId;
  }
}
