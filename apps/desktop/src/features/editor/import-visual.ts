import type { Command, ImageProbe, Project, PsdProbe } from '@sungeki/api-client';

export type VisualSource = { path: string } & (
  { kind: 'image'; probe: ImageProbe } | { kind: 'psd'; probe: PsdProbe }
);

// Builds one common-editor transaction; validation and committed state belong to the host.
export function importVisual(project: Project, source: VisualSource, startUs: number,
  ids: { asset: string; clip: string; track: string }): Command[] {
  const { probe, path } = source;
  const rotated = source.kind === 'image' && source.probe.orientation >= 5;
  const width = rotated ? probe.height : probe.width;
  const height = rotated ? probe.width : probe.height;
  const scale = Math.min(1, project.canvas.width / width, project.canvas.height / height);
  // Imported visuals are new front layers, including over an occupied image track.
  const commands: Command[] = [{ type: 'register_asset', asset: {
    id: ids.asset, name: path.split(/[\\/]/).at(-1) ?? (source.kind === 'psd' ? 'PSD立ち絵' : '画像'),
    locator: { type: 'absolute', path }, source: null, credit: '', content_hash: probe.sha256,
    probe: source.kind === 'image' ? { type: 'image', info: source.probe } : { type: 'psd', info: source.probe },
  } }];
  commands.push({ type: 'add_track', track: { id: ids.track,
    name: source.kind === 'psd' ? 'PSD立ち絵' : '画像', kind: 'visual', visible: true, muted: false } });
  commands.push({ type: 'add_clip', clip: {
    id: ids.clip, track_id: ids.track, scene_id: null, start_us: startUs, duration_us: 5_000_000,
    transform: { x: (project.canvas.width - width * scale) / 2, y: (project.canvas.height - height * scale) / 2,
      width: width * scale, height: height * scale, opacity: 1 },
    content: source.kind === 'image' ? { type: 'image', asset_id: ids.asset }
      : { type: 'psd', asset_id: ids.asset, pose: { source_sha256: probe.sha256, selections: [] }, speech: null },
  } });
  return commands;
}
