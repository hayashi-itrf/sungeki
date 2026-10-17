import { locale } from '../../shared/i18n/locale.svelte';
import { sceneFadeFromForm, type SceneFadeInput } from './scene-fade';
import { diagramMotionFromForm, type DiagramMotionInput } from './diagram-motion';
import { reactionFromForm, type ReactionInput } from './reaction';
import type { BuildDiagram, Project, VideoPlayback, VideoProbe } from '@sungeki/api-client';
export type DiagramInput = { asset: string; first: number | undefined; last: number | undefined; videoPlayback?: VideoPlayback; reaction?: ReactionInput; motion?: DiagramMotionInput; fade?: SceneFadeInput };
export function videoDuration(probe: VideoProbe): number {
  const c = probe.metadata.video.clock;
  if (c.start_pts === null || c.duration_ts === null || ![c.duration_ts, c.time_base.numerator, c.time_base.denominator].every(n => Number.isSafeInteger(n) && n > 0)) throw new Error(locale.text('productionValidationVideoClock'));
  const denominator = BigInt(c.time_base.denominator);
  const duration = Number((BigInt(c.duration_ts) * BigInt(c.time_base.numerator) * 1000000n + denominator - 1n) / denominator);
  if (!Number.isSafeInteger(duration) || duration <= 0 || duration > 604800000000) throw new Error(locale.text('productionValidationVideoDuration'));
  return duration;
}
export function selectDiagram(project: Project, row: DiagramInput, assetId: string): DiagramInput {
  const asset = project.assets.find(a => a.id === assetId);
  return { asset: assetId, first: row.first, last: row.last,
    ...(row.fade ? { fade: { ...row.fade } } : {}),
    ...(asset?.probe.type === 'image' && row.motion ? { motion: { ...row.motion } } : {}),
    ...(asset?.probe.type === 'image' && row.reaction ? { reaction: { ...row.reaction } } : {}),
    ...(asset?.probe.type === 'video' ? { videoPlayback: { source_start_us: 0, source_duration_us: videoDuration(asset.probe.info), looped: true, audio_stream: null, gain_db: 0 } } : {}) };
}
export function diagramsFromForm(project: Project, rows: DiagramInput[], count: number): BuildDiagram[] {
  if (rows.length > 200) throw new Error(locale.text('productionValidationDiagramCount'));
  const occupied = new Set<number>();
  return rows.map((row, index) => {
    const { first, last } = row;
    const asset = project.assets.find(a => a.id === row.asset);
    if (!asset || (asset.probe.type !== 'image' && asset.probe.type !== 'video')) throw new Error(locale.text('productionValidationDiagramAsset').replace('{index}', String(index + 1)));
    if (row.reaction && asset.probe.type !== 'image') throw new Error(locale.text('productionValidationReactionImage'));
    if (row.motion && (asset.probe.type !== 'image' || row.reaction)) throw new Error(locale.text('productionValidationMotionImage'));
    if (row.fade && row.reaction) throw new Error(locale.text('productionValidationFadeReaction'));
    const fade = sceneFadeFromForm(row.fade);
    const motion = row.motion ? diagramMotionFromForm(row.motion) : undefined;
    const reaction = row.reaction ? reactionFromForm(row.reaction) : undefined;
    let playback: VideoPlayback | undefined;
    if (asset.probe.type === 'video') {
      const p = row.videoPlayback;
      if (!p || !Number.isSafeInteger(p.source_start_us) || !Number.isSafeInteger(p.source_duration_us) || p.source_start_us < 0 || p.source_duration_us <= 0 || p.source_start_us + p.source_duration_us > videoDuration(asset.probe.info) || !Number.isFinite(p.gain_db) || p.gain_db < -60 || p.gain_db > 24 || (p.audio_stream !== null && !asset.probe.info.metadata.audio.some(a => a.index === p.audio_stream && a.clock.start_pts !== null))) throw new Error(locale.text('productionValidationDiagramPlayback').replace('{index}', String(index + 1)));
      playback = { ...p };
    }
    if (first === undefined || last === undefined || !Number.isInteger(first) || !Number.isInteger(last) || first < 1 || first > last || last > count) throw new Error(locale.text('productionValidationDiagramRange').replace('{index}', String(index + 1)).replace('{count}', String(count)));
    for (let i = first; !reaction && i <= last; i++) {
      if (occupied.has(i)) throw new Error(locale.text('productionValidationDiagramOverlap').replace('{line}', String(i)));
      occupied.add(i);
    }
    return { ...(fade ? { fade } : {}), asset_id: row.asset, first_line: first - 1, last_line: last - 1, ...(playback ? { video_playback: playback } : {}), ...(reaction ? { reaction } : {}), ...(motion ? { motion } : {}) };
  });
}
