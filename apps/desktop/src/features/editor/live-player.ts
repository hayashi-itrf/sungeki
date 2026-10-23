import { extraText } from '../../shared/i18n/extra-ui';
import type { HostApi, SessionSnapshot } from '@sungeki/api-client';
import { LiveAudioCache } from './live-audio-cache';
import { cueGain, cueOffset, liveAudioPlan, projectDuration, type LiveCue } from './live-audio-plan';
type Voice = { source: AudioBufferSourceNode; gain: GainNode; cue: LiveCue };
export class LivePlayer {
  readonly plan;
  readonly duration;
  private context: AudioContext | null = null;
  private cache: LiveAudioCache | null = null;
  private master: GainNode | null = null;
  private voices = new Map<string, Voice>();
  private token = 0;
  private frame = 0;
  private origin = 0;
  private position = 0;
  private running = false;
  private volume = 1;
  private disposed = false;
  private requested = false;
  constructor(private api: Pick<HostApi, 'session'>, private snapshot: SessionSnapshot,
    private update: (time: number, playing: boolean, buffering: boolean) => void,
    private fail: (error: unknown) => void) {
    this.plan = liveAudioPlan(snapshot.project); this.duration = projectDuration(snapshot.project);
  }
  private now() { return Math.min(this.duration, this.origin + (this.context?.currentTime ?? 0)); }
  setVolume(value: number) {
    this.volume = value;
    if (this.master && this.context) this.master.gain.setTargetAtTime(value, this.context.currentTime, .01);
  }
  private stopVoices() {
    for (const { source, gain } of this.voices.values()) { source.stop(); source.disconnect(); gain.disconnect(); }
    this.voices.clear();
  }
  pause() {
    if (this.running) this.position = this.now();
    this.running = false; this.requested = false; this.token++; cancelAnimationFrame(this.frame); this.stopVoices();
    if (!this.disposed) this.update(this.position, false, false);
  }
  async play(position: number) {
    if (this.disposed) return;
    this.pause();
    this.position = position >= this.duration ? 0 : Math.max(0, position);
    if (!this.duration) return;
    this.requested = true;
    const token = ++this.token;
    this.context ??= new AudioContext();
    this.cache ??= new LiveAudioCache(this.api, this.snapshot, this.context);
    this.cache.retry();
    this.master ??= this.context.createGain();
    this.master.gain.value = this.volume; this.master.disconnect(); this.master.connect(this.context.destination);
    try {
      await this.context.resume();
      if (token !== this.token) return;
      // Pause other audio/video preview controls without resetting their positions.
      document.dispatchEvent(new Event('vvm-editing-play'));
      this.origin = this.position - this.context.currentTime;
      this.running = true;
      await this.tick(token);
    } catch (error) { if (token === this.token) { this.pause(); this.fail(error); } }
  }
  seek(position: number) {
    if (this.disposed) return;
    const resume = this.requested;
    this.pause(); this.position = Math.max(0, Math.min(position, this.duration));
    this.update(this.position, false, false);
    if (resume && this.position < this.duration) void this.play(this.position);
  }
  private async tick(token: number) {
    if (token !== this.token || !this.running || !this.context || !this.cache || !this.master) return;
    try {
      let time = this.now();
      if (time >= this.duration) { this.position = this.duration; this.running = false; this.pause(); return; }
      const active = this.plan.cues.filter(c => c.start <= time && time < c.end);
      if (active.length > 32) throw new Error(extraText("編集再生で同時に再生できる音声は32個までです"));
      const missing = active.filter(c => !this.voices.has(c.clip.id) && !this.cache!.get(c.clip.id));
      if (missing.length) {
        // Hold the clock while decoding; never skip the beginning of a sentence.
        this.position = time; this.running = false; this.stopVoices();
        this.update(time, true, true);
        const loaded = new Map<string, AudioBuffer>();
        let decodedBytes = 0;
        for (const cue of active) {
          const buffer = await this.cache.load(cue);
          decodedBytes += buffer.length * buffer.numberOfChannels * 4;
          if (decodedBytes > 128 * 1024 * 1024) throw new Error(extraText("同時再生する音声が編集再生のメモリー上限を超えています"));
          loaded.set(cue.clip.id, buffer);
        }
        if (token !== this.token) return;
        this.origin = time - this.context.currentTime; this.running = true;
        for (const cue of active) this.startVoice(cue, time, loaded.get(cue.clip.id) ?? this.cache.get(cue.clip.id));
      }
      for (const [id, voice] of this.voices) {
        if (time >= voice.cue.end) { voice.source.stop(); voice.source.disconnect(); voice.gain.disconnect(); this.voices.delete(id); }
      }
      for (const cue of active) this.startVoice(cue, time, this.cache.get(cue.clip.id));
      for (const cue of this.plan.cues) {
        if (cue.start > time && cue.start < time + .15) this.startVoice(cue, cue.start, this.cache.get(cue.clip.id));
      }
      for (const voice of this.voices.values()) {
        voice.gain.gain.setTargetAtTime(cueGain(voice.cue, time, this.plan.cues), this.context.currentTime, .005);
      }
      this.position = time; this.update(time, true, false);
      // At most one nearby source is prefetched; failures are reported when needed.
      const next = this.plan.cues.find(c => c.start > time && c.start < time + 3 && !this.cache!.get(c.clip.id));
      if (next) void this.cache.load(next).catch(() => {});
      this.frame = requestAnimationFrame(() => { void this.tick(token); });
    } catch (error) { if (token === this.token) { this.pause(); this.fail(error); } }
  }
  private startVoice(cue: LiveCue, time: number, buffer: AudioBuffer | undefined) {
    if (!buffer || this.voices.has(cue.clip.id) || !this.context || !this.master) return;
    const source = this.context.createBufferSource(), gain = this.context.createGain();
    source.buffer = buffer;
    const p = cue.playback;
    if (p?.looped) { source.loop = true; source.loopStart = p.source_start_us / 1e6; source.loopEnd = (p.source_start_us + p.source_duration_us) / 1e6; }
    gain.gain.value = cueGain(cue, time, this.plan.cues);
    source.connect(gain); gain.connect(this.master);
    const offset = cueOffset(cue, time);
    source.start(this.context.currentTime + Math.max(0, cue.start - this.now()), Math.min(offset, buffer.duration), offset < buffer.duration ? cue.end - time : 0);
    this.voices.set(cue.clip.id, { source, gain, cue });
  }
  dispose() {
    if (this.disposed) return;
    // Teardown must not publish the old position into a newly opened document.
    this.disposed = true; this.pause(); this.cache?.dispose(); void this.context?.close();
  }
}
