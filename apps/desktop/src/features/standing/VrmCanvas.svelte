<script lang="ts">
  import { uiText } from '../../shared/i18n/extended-ui';
  import { untrack } from 'svelte';
  import type { Asset, VrmStanding } from '@sungeki/api-client';
  import type { VrmScene, PreviewView } from '@sungeki/vrm';
  import { hostApi } from '../../shared/host-api';
  import { readVrmBytes } from './vrm-bytes';
  let { standing, assets, sessionId, revision, timeUs = 0, width = 400, height = 550,
    view = { yaw: 0, pitch: 0, zoom: 1 }, generation = 0, onImage, onStatus = (_message: string) => {}, onBones = (_bones: string[]) => {} }: {
    standing: VrmStanding; assets: Asset[]; sessionId: string; revision: number;
    view?: PreviewView; timeUs?: number; width?: number; height?: number; generation?: number; onImage?: (png: string) => void; onStatus?: (message: string) => void; onBones?: (bones: string[]) => void;
  } = $props();
  let canvas = $state<HTMLCanvasElement>();
  let error = $state(''); let loading = $state(true);
  let active = $state.raw<VrmScene | null>(null);
  let model = $derived(assets.find(a => a.id === standing.model_asset_id));
  let motion = $derived(assets.find(a => a.id === standing.motion_asset_id));
  let identity = $derived(JSON.stringify([sessionId, model?.id, model?.content_hash,
    motion?.id, motion?.content_hash, generation, width, height]));
  $effect(() => {
    void identity;
    const target = canvas;
    if (!target) return;
    const input = untrack(() => ({ model, motion, revision, sessionId, width, height }));
    let disposed = false; let scene: VrmScene | undefined;
    loading = true; error = ''; active = null; onStatus(untrack(() => uiText("VRMを読み込んでいます…")));
    void (async () => {
      if (!input.model || !input.motion) throw Error(uiText("モデルまたはモーション素材がありません"));
      const module = await import('@sungeki/vrm');
      const modelBytes = await readVrmBytes(r => hostApi.session(r), input.sessionId, input.revision, input.model, () => disposed);
      const motionBytes = await readVrmBytes(r => hostApi.session(r), input.sessionId, input.revision, input.motion, () => disposed);
      if (disposed) return;
      target.width = Math.max(1, Math.min(2048, Math.ceil(input.width)));
      target.height = Math.max(1, Math.min(2048, Math.ceil(input.height)));
      scene = new module.VrmScene(target, modelBytes, motionBytes);
      const info = await scene.initialize();
      if (disposed) { scene.dispose(); return; }
      onBones(info.bones); active = scene; loading = false; onStatus('');
    })().catch(cause => { if (!disposed) { error = String(cause?.message ?? cause); loading = false; onStatus(error); scene?.dispose(); } });
    return () => { disposed = true; scene?.dispose(); };
  });
  let latest: { view: PreviewView; time: number; expressions: VrmStanding['expressions']; pose: NonNullable<VrmStanding['pose']> } | null = null;
  let running = false;
  async function render(scene: VrmScene) {
    if (running) return;
    running = true;
    try {
      while (latest && scene === active) {
        const frame = latest; latest = null;
        await scene.displayAt(frame.time, frame.expressions, frame.pose, () => !!latest || scene !== active, frame.view);
        if (scene === active && !latest && canvas) {
          onStatus(''); onImage?.(canvas.toDataURL('image/png'));
        }
      }
    } catch (cause) { if (scene === active && !latest) { error = String(cause); onStatus(error); } }
    finally { running = false; if (latest && active) void render(active); }
  }
  $effect(() => {
    const scene = active;
    latest = { view: $state.snapshot(view), time: timeUs, expressions: $state.snapshot(standing.expressions), pose: $state.snapshot(standing.pose ?? {}) };
    if (scene) { error = ''; void render(scene); }
  });
</script>
<div class="vrm-canvas" aria-label={uiText("VRMプレビュー")}>
  {#key identity}<canvas bind:this={canvas} aria-label={uiText("VRMモデル")} style:visibility={loading || error ? 'hidden' : 'visible'}></canvas>{/key}
  {#if loading || error}<p role={error ? 'alert' : 'status'}>{error || uiText("VRMを読み込んでいます…")}</p>{/if}
</div>
<style>
  .vrm-canvas { width:100%; height:100%; position:relative; }
  canvas { display:block; width:100%; height:100%; object-fit:contain; }
  p { position:absolute; inset:0; display:flex; align-items:center; justify-content:center; margin:0; padding:8px; font-size:14px; color:var(--text, #33443a); background:var(--panel-bg, #fff); overflow:auto; }
</style>
