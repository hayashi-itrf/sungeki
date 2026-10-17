import { tick } from 'svelte';
export function percentile(values: readonly number[], fraction: number): number | null {
  if (!values.length) return null;
  const sorted = [...values].sort((a, b) => a - b);
  return sorted[Math.max(0, Math.ceil(sorted.length * fraction) - 1)];
}
type EditSample = { responseMs: number; updateMs: number };
type Exclusion = 'hidden' | 'interrupted' | 'failed' | 'short' | 'noFrames';
const emptyExclusions = () => ({ hidden: 0, interrupted: 0, failed: 0, short: 0, noFrames: 0 });
class UiPerformance {
  enabled = $state(false);
  hidden = $state(false);
  edits = $state<EditSample[]>([]);
  dragIntervals = $state<number[]>([]);
  dragCount = $state(0);
  exclusions = $state(emptyExclusions());
  private pending = new Set<(reason?: Exclusion) => boolean>();
  private generation = 0;
  private frames = new Set<number>();
  private invalidate = (reason: Exclusion = 'interrupted') => {
    for (const finish of this.pending) finish(reason);
    this.generation++;
    for (const id of this.frames) cancelAnimationFrame(id);
    this.frames.clear();
  };
  private visibilityChanged = () => {
    this.hidden = document.hidden;
    if (this.hidden) this.invalidate('hidden');
  };
  private beginSample() {
    if (!this.enabled) return null;
    this.hidden = document.hidden;
    if (document.hidden) { this.exclusions.hidden++; return null; }
    const finish = (reason?: Exclusion) => {
      if (!this.pending.delete(finish)) return false;
      if (reason) this.exclusions[reason]++;
      return true;
    };
    this.pending.add(finish);
    return finish;
  }
  private frame(callback: (time: number) => void) {
    const id = requestAnimationFrame(time => { this.frames.delete(id); callback(time); });
    this.frames.add(id);
  }
  setEnabled(enabled: boolean) {
    this.invalidate(); this.enabled = enabled; this.hidden = document.hidden;
    document.removeEventListener('visibilitychange', this.visibilityChanged);
    if (enabled) document.addEventListener('visibilitychange', this.visibilityChanged);
  }
  clear() { this.setEnabled(this.enabled); this.edits = []; this.dragIntervals = []; this.dragCount = 0; this.exclusions = emptyExclusions(); }
  beginEdit(): ((success: boolean) => void) | null {
    const finish = this.beginSample(); if (!finish) return null;
    const generation = this.generation, start = performance.now();
    const unavailable = () => {
      if (generation !== this.generation) return true;
      if (document.hidden) { finish('hidden'); return true; }
      return false;
    };
    return success => {
      const responseMs = performance.now() - start;
      if (unavailable()) return;
      if (!success) { finish('failed'); return; }
      // Two frame boundaries after Svelte flush: an observation window, not a
      // guarantee that every async image/decoder has finished painting.
      void tick().then(() => {
        if (unavailable()) return;
        this.frame(() => {
          if (unavailable()) return;
          this.frame(() => {
            if (unavailable() || !finish()) return;
            this.edits = [...this.edits.slice(-199), { responseMs, updateMs: performance.now() - start }];
          });
        });
      });
    };
  }
  beginDrag(): (() => void) | null {
    const finish = this.beginSample(); if (!finish) return null;
    const generation = this.generation, start = performance.now();
    let done = false, invalid = false, previous: number | null = null;
    const intervals: number[] = [];
    const sample = (time: number) => {
      if (done || generation !== this.generation) return;
      if (document.hidden) { invalid = true; finish('hidden'); return; }
      if (previous !== null) { intervals.push(time - previous); if (intervals.length > 2000) intervals.shift(); }
      previous = time; this.frame(sample);
    };
    this.frame(sample);
    return () => {
      if (done) return;
      done = true;
      if (invalid || generation !== this.generation) return;
      if (document.hidden) { finish('hidden'); return; }
      if (performance.now() - start < 250) { finish('short'); return; }
      if (!intervals.length) { finish('noFrames'); return; }
      if (!finish()) return;
      this.dragCount++;
      this.dragIntervals = [...this.dragIntervals, ...intervals].slice(-2000);
    };
  }
}
export const uiPerformance = new UiPerformance();
