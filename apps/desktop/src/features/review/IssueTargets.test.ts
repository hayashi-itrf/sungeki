import { expect, test } from 'vitest';
import { render } from 'svelte/server';
import type { Issue, Project } from '@sungeki/api-client';
import IssueTargets from './IssueTargets.svelte';
import AssetDetails from '../editor/AssetDetails.svelte';
import type { EditorController } from '../editor/controller.svelte';
const issue: Issue = { severity: 'error', code: 'ASSET_MISSING', field: '/assets/0', asset_id: 'asset', clip_id: null, scene_id: null, start_us: null, end_us: null, message: '画像が見つかりません' };
function project(): Project {
  return { format: 'vvm-project', schema_version: 1, id: 'project', title: '診断',
    canvas: { width: 1920, height: 1080, fps: { numerator: 30, denominator: 1 }, background: '#ffffff' },
    tracks: [{ id: 'track', name: '映像', kind: 'visual', visible: true, muted: false }], clips: [], scenes: [], groups: [], characters: [], pronunciation_dictionary: [], audio_mix: { normalization: null },
    assets: [{ id: 'asset', name: '問題の画像', source: null, credit: '', locator: { type: 'absolute', path: '/missing.png' }, content_hash: '0'.repeat(64),
      probe: { type: 'image', info: { format: 'png', width: 1, height: 1, orientation: 1, has_alpha: true, byte_length: 100, sha256: '0'.repeat(64) } } }] };
}
const noop = () => {};
function view(p: Project, target = issue) { return render(IssueTargets, { props: { project: p, issue: target, onclip: noop, onasset: noop } }).body; }
test('unused assets remain repairable while deleted assets do not expose a misleading repair action', () => {
  const p = project();
  expect(view(p)).toContain('素材を確認・再指定');
  expect(view(p)).toContain('使っているクリップはありません');
  p.assets = [];
  expect(view(p)).toContain('対象素材は現在の作品にありません');
  expect(view(p)).not.toContain('素材を確認・再指定');
});
test('multiple uses are ordered by current time and paged instead of choosing an arbitrary clip', () => {
  const p = project();
  p.clips = Array.from({ length: 10 }, (_, i) => ({ id: `clip-${i}`, track_id: 'track', scene_id: null,
    start_us: (20 - i) * 1_000_000, duration_us: 1_000_000, transform: { x: 0, y: 0, width: 100, height: 100, opacity: 1 }, content: { type: 'image', asset_id: 'asset' } }));
  const result = view(p);
  expect(result).toContain('使用箇所（10個）');
  expect(result).toContain('続きを表示');
  expect(result).toContain('0:11.00へ移動');
  expect(result).not.toContain('0:20.00へ移動');
  const deletedClip = view(p, { ...issue, clip_id: 'deleted' });
  expect(deletedClip).toContain('対象クリップは現在の作品にありません');
  expect(deletedClip).not.toContain('素材を確認・再指定');
});
test('audio diagnostics list both material and synthesized speech uses with the shared repair action', () => {
  const p = project();
  p.assets[0].name = '問題の音声';
  p.assets[0].probe = { type: 'audio', info: { sha256: '0'.repeat(64), byte_length: 48044, sample_rate: 24000, channels: 1, frames: 24000, duration_us: 1000000 } };
  const clip = { id: 'speech', track_id: 'track', scene_id: null, start_us: 0, duration_us: 1000000, transform: { x: 0, y: 0, width: 100, height: 100, opacity: 1 } };
  p.clips = [
    { ...clip, content: { type: 'speech', character_id: 'speaker', subtitle_text: 'test', spoken_text: null, gain_db: null, audio: { asset_id: 'asset', input_fingerprint: '0'.repeat(64), engine_version: 'test', lip_sync: null } } },
    { ...clip, id: 'material', start_us: 2000000, content: { type: 'audio', asset_id: 'asset', playback: {} as never } },
  ];
  const html = view(p);
  expect(html).toContain('使用箇所（2個）');
  expect(html).not.toContain('使っているクリップはありません');
  expect(html).toContain('素材を確認・再指定');
  expect(html).not.toContain('再指定は未対応');
});
test('exact issue-time action is hidden for stale and out-of-range diagnostics', () => {
  const p = project();
  p.clips = [{ id: 'clip', track_id: 'track', scene_id: null, start_us: 0, duration_us: 2000000, transform: { x: 0, y: 0, width: 100, height: 100, opacity: 1 }, content: { type: 'image', asset_id: 'asset' } }];
  const target = { ...issue, clip_id: 'clip', start_us: 1500000 };
  expect(view(p, target)).toContain('指摘の時刻へ移動（0:01.50）');
  expect(view(p, { ...target, start_us: 2000000 })).not.toContain('指摘の時刻へ移動');
  const stale = render(IssueTargets, { props: { project: p, issue: target, stale: true, onclip: noop, onasset: noop } }).body;
  expect(stale).not.toContain('指摘の時刻へ移動');
  expect(stale).toContain('対象クリップへ移動');
});

test('missing sprite images show their clip use instead of claiming the asset is unused', () => {
  const p = project();
  p.clips = [{ id: 'sprite', track_id: 'track', scene_id: null, start_us: 2_000_000, duration_us: 1_000_000,
    transform: { x: 0, y: 0, width: 100, height: 100, opacity: 1 },
    content: { type: 'sprite', character_id: null, pose: null, sprites: { default_pose: 'normal', poses: [
      { name: 'normal', closed_asset_id: 'asset', open_asset_id: 'asset' },
    ] } } }];
  const html = view(p);
  expect(html).toContain('使用箇所（1個）');
  expect(html).toContain('0:02.00へ移動');
  expect(html).toContain('素材を確認・再指定');
  expect(html).not.toContain('使っているクリップはありません');
});

test('repair impact counts image and sprite clips once each, including inactive poses', () => {
  const p = project();
  const clip = { id: 'sprite', track_id: 'track', scene_id: null, start_us: 0, duration_us: 1_000_000,
    transform: { x: 0, y: 0, width: 100, height: 100, opacity: 1 } };
  p.clips = [
    { ...clip, content: { type: 'sprite', character_id: null, pose: 'normal', sprites: {
      default_pose: 'normal', poses: [
        { name: 'normal', closed_asset_id: 'other', open_asset_id: null },
        { name: 'alternate', closed_asset_id: 'asset', open_asset_id: 'asset' },
      ],
    } } },
    { ...clip, id: 'image', content: { type: 'image', asset_id: 'asset' } },
    { ...clip, id: 'unrelated', content: { type: 'image', asset_id: 'other' } },
  ];
  const editor = { snapshot: { project: p }, busy: false, pending: null, drafting: false } as unknown as EditorController;
  const html = render(AssetDetails, { props: { editor, assetId: 'asset' } }).body;
  expect(html).toContain('2個のクリップに反映');
  expect(view(p)).toContain('使用箇所（2個）');
});


test('scene-only diagnostics navigate by current scene identity and never offer deleted scenes', () => {
  const p = project();
  p.scenes = [{ id: 'scene', name: '導入', start_us: 12_000_000, duration_us: 3_000_000 }];
  const target = { ...issue, asset_id: null, clip_id: null, scene_id: 'scene', start_us: 2_000_000 };
  const props = { project: p, issue: target, onclip: noop, onasset: noop, onscene: noop };
  const html = render(IssueTargets, { props }).body;
  expect(html).toContain('シーン「導入」へ移動（0:12.00）');
  expect(html).not.toContain('0:02.00');
  const stale = render(IssueTargets, { props: { ...props, stale: true } }).body;
  expect(stale).toContain('現在のシーン「導入」へ移動（0:12.00）');
  const missing = render(IssueTargets, { props: { ...props, project: { ...p, scenes: [] } } }).body;
  expect(missing).toContain('対象シーンは現在の作品にありません');
  expect(missing).not.toContain('<button');
  const disabled = render(IssueTargets, { props: { ...props, disabled: true } }).body;
  expect(disabled).toContain('disabled');
});
