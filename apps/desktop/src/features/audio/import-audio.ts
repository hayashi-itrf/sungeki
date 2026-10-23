import { extraText } from '../../shared/i18n/extra-ui';
import type { AudioProbe, Command } from '@sungeki/api-client';
export function importAudio(path: string, probe: AudioProbe, start: number,
  ids: { asset: string; track: string; clip: string }): Command[] {
  if (!path.startsWith('/') || !Number.isSafeInteger(start) || start < 0 ||
      !Number.isSafeInteger(probe.duration_us) || probe.duration_us <= 0 || start + probe.duration_us > 604800000000) {
    throw new Error(extraText("音声の配置範囲が作品の上限を超えています。開始位置を変更してください。"));
  }
  const name = path.split('/').at(-1) || '音声素材';
  return [
    { type: 'register_asset', asset: { id: ids.asset, name, source: null, credit: '',
      locator: { type: 'absolute', path }, probe: { type: 'audio', info: probe }, content_hash: probe.sha256 } },
    { type: 'add_track', track: { id: ids.track, name, kind: 'audio', visible: true, muted: false } },
    { type: 'add_clip', clip: { id: ids.clip, track_id: ids.track, scene_id: null, start_us: start,
      duration_us: probe.duration_us, transform: { x: 0, y: 0, width: 1, height: 1, opacity: 1 },
      content: { type: 'audio', asset_id: ids.asset, playback: { source_start_us: 0,
        source_duration_us: probe.duration_us, looped: false, gain_db: 0, volume_points: [], fade_in_us: 0, ducking: null, fade_out_us: 0 } } } },
  ];
}
