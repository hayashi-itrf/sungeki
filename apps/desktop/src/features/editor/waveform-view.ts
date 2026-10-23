import { extraText } from '../../shared/i18n/extra-ui';
import type { AnalyzeAudioArgs, AudioAnalysis, AudioProbe, Clip } from '@sungeki/api-client';
export function waveformRange(clip: Clip, probe: AudioProbe, start: number, span: number) {
  const from = Math.max(0, (start * 1e6 - clip.start_us) * probe.sample_rate / 1e6);
  const clipEnd = Math.min(clip.duration_us * probe.sample_rate / 1e6,
    ((start + span) * 1e6 - clip.start_us) * probe.sample_rate / 1e6);
  const to = Math.min(probe.frames, clipEnd);
  if (!Number.isFinite(from + to) || to <= from) return null;
  const level = Math.max(0, Math.ceil(Math.log2((to - from) / (256 * 256))));
  const width = 256 * 2 ** level;
  const first = Math.floor(from / width), end = Math.ceil(to / width);
  return { from, to, level, first, count: end - first, width, fraction: (to - from) / (clipEnd - from) };
}
export function validateWaveform(a: AudioAnalysis, r: AnalyzeAudioArgs, p: AudioProbe) {
  const width = r.base_frames * 2 ** r.level;
  const count = Math.min(r.count, Math.ceil(p.frames / width) - r.start_bucket);
  if (a.session_id !== r.session_id || a.source_revision !== r.expected_revision || a.asset_id !== r.asset_id
    || a.source_sha256 !== p.sha256 || a.version !== 1 || a.signal !== 'source_pre_gain'
    || a.sample_rate !== p.sample_rate || a.source_frames !== p.frames || a.channels !== p.channels
    || a.level !== r.level || a.channel !== r.channel || a.start_bucket !== r.start_bucket
    || a.start_frame !== Math.min(p.frames, r.start_bucket * width)
    || a.end_frame !== Math.min(p.frames, (r.start_bucket + count) * width)
    || a.peaks.length !== count || a.peaks.some(([min, max]) => !Number.isFinite(min + max) || min > max)) {
    throw new Error(extraText("音声素材と波形の応答が一致しません。"));
  }
}
export function waveformPath(a: AudioAnalysis, range: NonNullable<ReturnType<typeof waveformRange>>) {
  return a.peaks.map(([min, max], i) => {
    const left = Math.max(range.from, (a.start_bucket + i) * range.width);
    const right = Math.min(range.to, (a.start_bucket + i + 1) * range.width);
    const x = ((left + right) / 2 - range.from) * 1000 / (range.to - range.from);
    // Fixed full-scale display, never normalize quiet clips to look loud.
    const y = (v: number) => 10 - Math.max(-1, Math.min(1, v)) * 9;
    return `M${x.toFixed(2)},${y(max).toFixed(2)}V${y(min).toFixed(2)}`;
  }).join('');
}
let queue = Promise.resolve();
export function queueWaveform(task: () => Promise<void>) {
  const next = queue.then(task, task);
  queue = next.catch(() => {});
  return next;
}
