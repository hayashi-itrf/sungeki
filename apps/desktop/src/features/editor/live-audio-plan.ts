import type { AudioPlayback, Clip, Project } from '@sungeki/api-client';
export type LiveCue = { clip: Clip; assetId: string; start: number; end: number; gain: number; playback: AudioPlayback | null; speechEnd: number };
export function liveAudioPlan(project: Project): { cues: LiveCue[]; missing: number } {
  const muted = new Set(project.tracks.filter(t => t.muted).map(t => t.id));
  const cues: LiveCue[] = []; let missing = 0;
  for (const clip of project.clips) {
    if (muted.has(clip.track_id)) continue;
    const c = clip.content;
    if (c.type === 'speech' && !c.audio) { missing++; continue; }
    if (c.type === 'video' && c.playback.audio_stream === null) continue;
    if (c.type !== 'audio' && c.type !== 'speech' && c.type !== 'video') continue;
    cues.push({ clip, assetId: c.type === 'speech' ? c.audio!.asset_id : c.asset_id,
      start: clip.start_us / 1e6, end: (clip.start_us + clip.duration_us) / 1e6,
      speechEnd: (clip.start_us + (c.type === 'speech' ? (() => {
        const asset = project.assets.find(a => a.id === c.audio!.asset_id);
        return asset?.probe.type === 'audio' ? asset.probe.info.duration_us : clip.duration_us;
      })() : clip.duration_us)) / 1e6,
      gain: 10 ** ((c.type === 'speech' ? c.gain_db ?? 0 : c.playback.gain_db) / 20),
      playback: c.type === 'audio' ? c.playback : c.type === 'video' ? { ...c.playback, volume_points: [], fade_in_us: 0, fade_out_us: 0, ducking: null } : null });
  }
  return { cues, missing };
}
export function cueOffset(cue: LiveCue, time: number): number {
  const p = cue.playback, elapsed = Math.max(0, time - cue.start);
  return p ? p.source_start_us / 1e6 + (p.looped ? elapsed % (p.source_duration_us / 1e6) : elapsed) : elapsed;
}
export function cueGain(cue: LiveCue, time: number, cues: LiveCue[]): number {
  const p = cue.playback;
  if (!p) return cue.gain;
  const elapsed = (time - cue.start) * 1e6, duration = (cue.end - cue.start) * 1e6;
  let level = cue.gain;
  if (p.fade_in_us) level *= Math.min(1, Math.max(0, elapsed / p.fade_in_us));
  if (p.fade_out_us) level *= Math.min(1, Math.max(0, (duration - elapsed) / p.fade_out_us));
  const points = p.volume_points;
  if (points.length) {
    const right = points.findIndex(v => v.at_us > elapsed);
    if (right === 0) level *= points[0].level;
    else if (right < 0) level *= points[points.length - 1].level;
    else { const a = points[right - 1], b = points[right]; level *= a.level + (b.level - a.level) * (elapsed - a.at_us) / (b.at_us - a.at_us); }
  }
  if (p.ducking) {
    let reduction = 0;
    for (const speech of cues) {
      if (speech.clip.content.type !== 'speech') continue;
      const attack = p.ducking.attack_us / 1e6, release = p.ducking.release_us / 1e6;
      const amount = time < speech.start ? (attack ? 1 - (speech.start - time) / attack : 0)
        : time < speech.speechEnd ? 1 : release ? 1 - (time - speech.speechEnd) / release : 0;
      reduction = Math.max(reduction, Math.min(1, amount));
    }
    level *= 1 + (10 ** (p.ducking.attenuation_db / 20) - 1) * reduction;
  }
  return level;
}
export function projectDuration(project: Project): number {
  return Math.max(0, ...project.clips.map(c => (c.start_us + c.duration_us) / 1e6),
    ...project.scenes.map(s => (s.start_us + s.duration_us) / 1e6));
}
