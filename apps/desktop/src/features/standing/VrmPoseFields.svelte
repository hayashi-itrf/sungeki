<script lang="ts">
  import { untrack } from 'svelte';
  import type { Asset, VrmStanding } from '@sungeki/api-client';
  import { containDialogFocus } from '../../shared/components/dialog-focus';
  import VrmCanvas from './VrmCanvas.svelte';
  let { value, assets, sessionId, revision, commit, beginPose, durationUs = 1, initialTimeUs = 0, disabled = false, blocked = false, error = '' }: {
    value: VrmStanding; assets: Asset[]; sessionId: string; revision: number; commit: (pose: NonNullable<VrmStanding['pose']>) => Promise<boolean>; durationUs?: number; initialTimeUs?: number; disabled?: boolean; blocked?: boolean; error?: string; beginPose?: () => (() => void);
  } = $props();
  const joints = [
    ['hips','腰'],['spine','背骨'],['chest','胸'],['upperChest','胸の上'],['neck','首'],['head','頭'],
    ['leftShoulder','左肩'],['leftUpperArm','左上腕'],['leftLowerArm','左ひじ'],['leftHand','左手首'],
    ['rightShoulder','右肩'],['rightUpperArm','右上腕'],['rightLowerArm','右ひじ'],['rightHand','右手首'],
    ['leftUpperLeg','左もも'],['leftLowerLeg','左ひざ'],['leftFoot','左足首'],
    ['rightUpperLeg','右もも'],['rightLowerLeg','右ひざ'],['rightFoot','右足首'],
  ];
  let dialog: HTMLDialogElement;
  let draft = $state<VrmStanding>(untrack(() => $state.snapshot(value)));
  let discard: (() => void) | undefined;
  function close() { open = false; const reset = discard; discard = undefined; reset?.(); }
  function show() { discard = beginPose?.(); available = []; draft = structuredClone($state.snapshot(value)); at = Math.min(Math.max(0, durationUs - 1), initialTimeUs) / 1e6; view = { yaw: 0, pitch: 0, zoom: 1 }; open = true; dialog.showModal(); }
  let at = $state(0);
  let view = $state({ yaw: 0, pitch: 0, zoom: 1 });
  let busy = $state(false);
  async function usePose() {
    if (busy || blocked || ![...dialog.querySelectorAll<HTMLInputElement>('input')].every(input => input.reportValidity())) return;
    busy = true;
    try { if (await commit($state.snapshot(draft.pose ?? {}))) { discard = undefined; dialog.close(); } }
    finally { busy = false; }
  }
  let open = $state(false); let bone = $state('leftUpperArm'); let available = $state<string[]>([]);
  let angles = $derived(draft.pose?.[bone] ?? [0, 0, 0]);
  function update(axis: number, angle: number) {
    if (!Number.isFinite(angle) || Math.abs(angle) > 180) return;
    const next = [...angles] as [number, number, number]; next[axis] = angle;
    draft = { ...draft, pose: { ...draft.pose, [bone]: next } };
  }
  function reset(all = false) {
    const pose = { ...draft.pose }; delete pose[bone];
    draft = { ...draft, pose: all ? {} : pose };
  }
</script>
<button type="button" {disabled} onclick={show}>姿を見ながらポーズを編集</button>
<dialog bind:this={dialog} use:containDialogFocus aria-label="VRMのポーズを編集"
  onkeydown={event => { if (event.key === 'Enter' && !(event.target instanceof HTMLButtonElement)) event.preventDefault(); }}
  oncancel={event => { if (busy) event.preventDefault(); }} oninput={event => event.stopPropagation()} onclose={close}>
  <header><h2>VRMのポーズを編集</h2></header>
  {#if open}
    <div class="layout"><div class="stage"><VrmCanvas standing={draft} {assets} {sessionId} {revision}
      timeUs={value.source_start_us + Math.round((Number.isFinite(at) ? Math.min(Math.max(0, at), Math.max(0, durationUs - 1) / 1e6) : 0) * 1e6)} {view} onBones={bones => { available = bones; }} /></div><fieldset class="controls" disabled={busy}>
    <p>確認時刻と視点はこの画面だけに使用します。作品の時刻・カメラ設定は変わりません。</p>
    <label>クリップ内の確認時刻（秒）<input type="range" min="0" max={Math.max(0, durationUs - 1) / 1e6} step="any" bind:value={at} /></label>
    <label>確認時刻（秒）<input type="number" required min="0" max={Math.max(0, durationUs - 1) / 1e6} step="any" bind:value={at} /></label>
    <details><summary>拡大・視点</summary>
      <label>拡大倍率 {view.zoom.toFixed(1)}倍<input type="range" min="1" max="4" step="0.1" bind:value={view.zoom} /></label>
      <label>横から見る角度 {view.yaw}°<input type="range" min="-180" max="180" step="1" bind:value={view.yaw} /></label>
      <label>上下から見る角度 {view.pitch}°<input type="range" min="-80" max="80" step="1" bind:value={view.pitch} /></label>
      <button type="button" onclick={() => { view = { yaw: 0, pitch: 0, zoom: 1 }; }}>正面・全身に戻す</button>
    </details>
    <p>角度を変えた関節だけモーションより優先されます。</p>
    <label>関節<select bind:value={bone}>
      {#each joints as [id, name]}<option value={id} disabled={!available.includes(id)}>{name}{available.length && !available.includes(id) ? '（なし）' : ''}</option>{/each}
    </select></label>
    <p>{draft.pose?.[bone] ? 'この関節は指定した角度で固定します。' : 'この関節はモーションに従っています。角度を入力すると基準姿勢からの角度で固定します。'}</p>
    {#each ['X', 'Y', 'Z'] as axis, index}
      <label>{axis}軸（度）
        <input type="range" min="-180" max="180" step="1" value={angles[index]}
          disabled={!available.includes(bone)} oninput={e => update(index, e.currentTarget.valueAsNumber)} />
        <input aria-label={`${axis}軸の角度`} type="number" required min="-180" max="180" step="1" value={angles[index]}
          disabled={!available.includes(bone)} oninput={e => update(index, e.currentTarget.valueAsNumber)} />
      </label>
    {/each}
    <button type="button" onclick={() => reset()}>この関節をモーションに戻す</button>
    <button type="button" onclick={() => reset(true)}>全関節をモーションに戻す</button>
    </fieldset></div>
    <footer>
      <div><p>「ポーズを適用」で作品に反映します。「元に戻す」で戻せます。</p>
      {#if blocked}<p role="alert">別の更新または未解決の送信があります。閉じて編集画面の案内を確認してください。</p>{/if}
      {#if error}<p role="alert">{error}</p>{/if}</div>
      <button type="button" disabled={busy} onclick={() => dialog.close()}>取り消す</button>
      <button type="button" onclick={usePose} disabled={busy || blocked || !available.length}>{busy ? '適用中…' : 'ポーズを適用'}</button>
    </footer>
  {/if}
</dialog>
<style>
  dialog { width:min(840px, calc(100vw - 48px)); max-height:calc(100dvh - 32px); box-sizing:border-box; overflow:hidden; }
  dialog[open] { display:flex; flex-direction:column; }
  header { display:flex; align-items:center; justify-content:space-between; gap:12px; }
  h2 { margin:0; font-size:18px; }
  .layout { display:grid; grid-template-columns:minmax(180px,1fr) minmax(220px,1fr); gap:20px; margin-top:16px; min-height:0; overflow:auto; }
  .controls { min-width:0; margin:0; border:0; overflow:auto; padding:0 6px; }
  .stage { height:min(500px, 55dvh); position:sticky; top:0; border:1px solid var(--panel-border); background:var(--panel-bg); }
  footer { flex-shrink:0; display:flex; align-items:center; gap:10px; border-top:1px solid var(--panel-border); padding-top:10px; margin-top:12px; }
  footer div { flex:1; } footer button { white-space:nowrap; }
  @media(max-width:600px) { .layout { grid-template-columns:1fr; } .stage { position:relative; height:240px; } footer { flex-wrap:wrap; } footer div { flex-basis:100%; } }
  label { display:flex; flex-direction:column; gap:4px; margin:8px 0; font-size:12px; }
  input,select { width:100%; box-sizing:border-box; } p { font-size:11px; line-height:1.5; }
</style>
