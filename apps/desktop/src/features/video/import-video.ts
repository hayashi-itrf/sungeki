import { locale } from '../../shared/i18n/locale.svelte';
import type { Canvas, Command, VideoProbe, VideoStreamInfo } from '@sungeki/api-client';
// Initial placement only. Actual decoding/display normalization belongs to the renderer.
export function displaySize(s: VideoStreamInfo): [number, number] {
  let a: number, b: number, c: number, d: number;
  if (s.display_matrix !== null) {
    const rows = s.display_matrix.trim().split('\n').map(row => row.split(':')[1]?.trim().split(/\s+/).map(Number));
    if (rows.length !== 3 || rows.some(row => !row || row.length !== 3)) throw new Error(locale.text('videoMatrixReadError'));
    const m = rows.flat() as number[];
    if (m.some(n => !Number.isInteger(n)) || m[2] !== 0 || m[5] !== 0 || m[6] !== 0 || m[7] !== 0 || m[8] !== 1073741824) throw new Error(locale.text('videoMatrixUnsupported'));
    [a, b, c, d] = [m[0], m[1], m[3], m[4]].map(n => n / 65536);
  } else {
    const angle = s.rotation_degrees ?? (s.legacy_rotate_tag === null ? 0 : Number(s.legacy_rotate_tag));
    const snap = (n: number) => Math.abs(n) < 1e-12 ? 0 : n;
    const sin = snap(Math.sin(angle * Math.PI / 180)), cos = snap(Math.cos(angle * Math.PI / 180));
    [a, b, c, d] = [cos, -sin, sin, cos];
  }
  const sar = s.sample_aspect_ratio ? s.sample_aspect_ratio.numerator / s.sample_aspect_ratio.denominator : 1;
  a *= sar; b *= sar;
  const width = Math.ceil(Math.abs(a * s.width) + Math.abs(c * s.height));
  const height = Math.ceil(Math.abs(b * s.width) + Math.abs(d * s.height));
  if (![width, height, a * d - b * c].every(Number.isFinite) || Math.abs(a * d - b * c) < 1e-9 || width < 1 || height < 1 || width > 16384 || height > 16384 || width * height * 4 > 64 * 1024 * 1024) throw new Error(locale.text('videoDimensionsUnsupported'));
  return [width, height];
}
export function importVideo(path: string, probe: VideoProbe, start: number, canvas: Canvas,
  ids: { asset: string; track: string; clip: string }): Command[] {
  const clock = probe.metadata.video.clock;
  if (!path.startsWith('/') || !Number.isSafeInteger(start) || start < 0 || clock.start_pts === null ||
      clock.duration_ts === null || ![clock.duration_ts, clock.time_base.numerator, clock.time_base.denominator].every(n => Number.isSafeInteger(n) && n > 0)) throw new Error(locale.text('videoTimingUnknown'));
  const denominator = BigInt(clock.time_base.denominator);
  const duration = Number((BigInt(clock.duration_ts) * BigInt(clock.time_base.numerator) * 1000000n + denominator - 1n) / denominator);
  if (!Number.isSafeInteger(duration) || duration <= 0 || start + duration > 604800000000) throw new Error(locale.text('videoRangeExceeded'));
  const [w, h] = displaySize(probe.metadata.video);
  const scale = Math.min(canvas.width / w, canvas.height / h, 1);
  const width = w * scale, height = h * scale;
  const name = path.split('/').at(-1) || '動画素材';
  return [
    { type: 'register_asset', asset: { id: ids.asset, name, source: null, credit: '', content_hash: probe.sha256,
      locator: { type: 'absolute', path }, probe: { type: 'video', info: probe } } },
    { type: 'add_track', track: { id: ids.track, name, kind: 'visual', visible: true, muted: false } },
    { type: 'add_clip', clip: { id: ids.clip, track_id: ids.track, scene_id: null, start_us: start, duration_us: duration,
      transform: { x: (canvas.width - width) / 2, y: (canvas.height - height) / 2, width, height, opacity: 1 },
      content: { type: 'video', asset_id: ids.asset, playback: { source_start_us: 0, source_duration_us: duration,
        looped: false, audio_stream: null, gain_db: 0 } } } },
  ];
}
