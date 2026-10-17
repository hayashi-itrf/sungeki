import type { AudioAnalysis, AudioProbe, Clip } from '@sungeki/api-client';

// Source-pre-gain envelopes. The source period uses the same ceil sample
// boundaries as the renderer; loop count never controls allocation or requests.
export function materialWaveformRange(clip: Clip, probe: AudioProbe, start: number, span: number) {
  if (clip.content.type !== 'audio' || !Number.isFinite(start + span) || span <= 0) return null;
  const p = clip.content.playback;
  const boundary = (us: number) => Number((BigInt(us) * BigInt(probe.sample_rate) + 999999n) / 1000000n);
  const first = boundary(p.source_start_us);
  const end = Math.min(probe.frames, boundary(p.source_start_us + p.source_duration_us));
  const period = end - first;
  const visibleStart = Math.max(0, start * 1e6 - clip.start_us);
  const visibleEnd = Math.min(clip.duration_us, (start + span) * 1e6 - clip.start_us);
  const from = visibleStart / 1e6 * probe.sample_rate;
  const fullEnd = visibleEnd / 1e6 * probe.sample_rate;
  const to = p.looped ? fullEnd : Math.min(fullEnd, period);
  if (period <= 0 || to <= from) return null;
  const sourceFrom = p.looped ? first : first + from;
  const sourceTo = p.looped ? end : first + to;
  const base = 2 ** Math.max(0, Math.ceil(Math.log2(probe.frames / 262144)));
  const level = Math.max(0, Math.ceil(Math.log2((sourceTo - sourceFrom) / (256 * base))));
  const width = base * 2 ** level;
  const bucket = Math.floor(sourceFrom / width);
  return { from, to, first, end, period, looped: p.looped, width, level, base,
    bucket, count: Math.ceil(sourceTo / width) - bucket, fraction: (to - from) / (fullEnd - from) };
}
export function materialWaveformPath(analysis: AudioAnalysis, r: NonNullable<ReturnType<typeof materialWaveformRange>>) {
  function peaks(from: number, to: number): [number, number] {
    const first = Math.max(0, Math.floor(from / r.width) - analysis.start_bucket);
    const end = Math.min(analysis.peaks.length, Math.ceil(to / r.width) - analysis.start_bucket);
    let min = Infinity, max = -Infinity;
    for (let i = first; i < end; i++) {
      min = Math.min(min, analysis.peaks[i][0]); max = Math.max(max, analysis.peaks[i][1]);
    }
    if (!Number.isFinite(min + max)) throw new Error('音声区間に対応する波形がありません。');
    return [min, max];
  }
  const paths: string[] = [];
  const columns = 256;
  const step = (r.to - r.from) / columns;
  const y = (v: number) => 10 - Math.max(-1, Math.min(1, v)) * 9;
  for (let i = 0; i < columns; i++) {
    const at = r.from + i * step;
    let pair: [number, number];
    if (!r.looped) pair = peaks(r.first + at, r.first + Math.min(r.to, at + step));
    else if (step >= r.period) pair = peaks(r.first, r.end);
    else {
      const position = at % r.period;
      pair = peaks(r.first + position, Math.min(r.end, r.first + position + step));
      if (position + step > r.period) {
        const other = peaks(r.first, r.first + position + step - r.period);
        pair = [Math.min(pair[0], other[0]), Math.max(pair[1], other[1])];
      }
    }
    paths.push(`M${((i + .5) * 1000 / columns).toFixed(2)},${y(pair[1]).toFixed(2)}V${y(pair[0]).toFixed(2)}`);
  }
  return paths.join('');
}
