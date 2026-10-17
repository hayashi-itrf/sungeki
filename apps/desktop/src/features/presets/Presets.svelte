<script lang="ts">
  import { containDialogFocus } from '../../shared/components/dialog-focus';
  import type { PresetRecord, PresetRequest, PresetSummary } from '@sungeki/api-client';
  import type { EditorController } from '../editor/controller.svelte';
  import { hostApi } from '../../shared/host-api';
  import { locale } from '../../shared/i18n/locale.svelte';
  import ApplyVoicePreset from './ApplyVoicePreset.svelte';
  import { captureVoice, captureCharacter, isStandingFor } from './capture';
  let { editor }: { editor: EditorController } = $props();
  const t = (ja: string, en: string) => locale.current === 'en' ? en : ja;
  let dialog: HTMLDialogElement;
  let busy = $state(false), error = $state(''), notice = $state('');
  let items = $state<PresetSummary[]>([]), next = $state<string | null>(null);
  let selected = $state<PresetRecord | null>(null);
  let pending = $state<PresetRequest | null>(null);
  let characterId = $state(''), clipId = $state(''), name = $state(''), trackId = $state('');
  let kind = $state<'character' | 'voice'>('character');
  let side = $state<'left' | 'right'>('left');
  let seconds = $state(5), deleteConfirm = $state(false), updateConfirm = $state(false);
  const locked = $derived(busy || editor.busy || editor.drafting || !!editor.pending || !!pending);
  const project = $derived(editor.snapshot?.project);
  const tracks = $derived(project?.tracks.filter(v => v.kind === 'visual' && v.visible) ?? []);
  async function request(value: PresetRequest, append = false) {
    busy = true; error = ''; notice = '';
    try {
      const result = await hostApi.session({ type: 'presets', request: value });
      pending = null;
      if (result.type === 'presets') { items = append ? [...items, ...result.items] : result.items; next = result.next_after_id; }
      if (result.type === 'preset') {
        selected = result.record.preset ? result.record : null;
        if (value.action === 'save' || value.action === 'delete') {
          const list = await hostApi.session({ type: 'presets', request: { action: 'list', after_id: null, limit: 20 } });
          if (list.type === 'presets') { items = list.items; next = list.next_after_id; }
          notice = t('保存内容を更新しました。作品本体は変更していません。', 'Library updated. The project is unchanged.');
        }
      }
    } catch (e) {
      error = e && typeof e === 'object' && 'message' in e ? String(e.message) : String(e);
      pending = null;
      const code = e && typeof e === 'object' && 'code' in e ? e.code : undefined;
      if ((value.action === 'save' || value.action === 'delete') && (!code || code === 'IO_ERROR' || code === 'HOST_UNAVAILABLE')) pending = value;
    } finally { busy = false; }
  }
  function open() { dialog.showModal(); void request({ action: 'list', after_id: null, limit: 20 }); }
  function save(update = false) {
    if (!project || locked) return;
    if (update && (!selected?.preset || selected.preset.content.kind !== kind)) return;
    updateConfirm = false;
    try {
      const preset = kind === 'voice' ? captureVoice(project, characterId, name) : captureCharacter(project, characterId, clipId, name);
      if (update && selected?.preset) { preset.description = selected.preset.description; preset.tags = [...selected.preset.tags]; }
      void request({ action: 'save', id: update ? selected!.id : crypto.randomUUID(), expected_revision: update ? selected!.revision : 0, preset });
    }
    catch (e) { error = String(e); }
  }
  async function apply() {
    const snapshot = editor.snapshot, record = selected;
    if (!snapshot || !record?.preset || locked || record.preset.content.kind !== 'character') return;
    notice = ''; error = '';
    const content = record.preset.content;
    await editor.request({ type: 'instantiate_preset', args: {
      session_id: snapshot.session_id, expected_revision: snapshot.revision, operation_id: crypto.randomUUID(),
      preset_id: record.id, preset_revision: record.revision,
      ids: { character_id: crypto.randomUUID(), asset_ids: Object.fromEntries(content.assets.map(a => [a.id, crypto.randomUUID()])) },
      placement: content.standing ? { clip_id: crypto.randomUUID(), track_id: trackId || crypto.randomUUID(), new_track_name: trackId ? undefined : t('立ち絵', 'Standing'), scene_id: null,
        start_us: Math.round(editor.time * 1_000_000), duration_us: Math.round(seconds * 1_000_000),
        transform: { x: side === 'left' ? 0 : snapshot.project.canvas.width / 2, y: snapshot.project.canvas.height * 0.1, width: snapshot.project.canvas.width / 2, height: snapshot.project.canvas.height * 0.7, opacity: 1 } } : null,
    } });
    notice = editor.error ? '' : t('作品へ追加しました。Undoで戻せます。', 'Added to the project. You can undo this.');
  }
</script>
<button disabled={!editor.snapshot || editor.busy || editor.drafting || !!editor.pending} onclick={open}>{t('人物・声プリセット', 'Character and voice presets')}</button>
<dialog use:containDialogFocus bind:this={dialog} aria-label={t('人物・声プリセット', 'Character and voice presets')} oncancel={e => { if (busy || pending || editor.busy || editor.pending) e.preventDefault(); }}>
  <header><h2>{t('人物・声プリセット', 'Character and voice presets')}</h2><button disabled={busy || !!pending || editor.busy || !!editor.pending} onclick={() => dialog.close()}>{t('閉じる', 'Close')}</button></header>
  <p>{t('声・字幕・立ち絵の設定を別の作品で再利用できます。素材本体はコピーしません。', 'Reuse voice, subtitles and standing settings across projects. Media files are not copied.')}</p>
  {#if error || editor.error}<p role="alert">{error || editor.error}</p>{/if}
  {#if notice}<p role="status">{notice}</p>{/if}
  {#if pending}<button disabled={busy} onclick={() => request(pending!)}>{t('同じ保存操作を再確認', 'Retry the same library operation')}</button>{/if}
  {#if editor.pending}<button disabled={editor.busy} onclick={() => editor.request(editor.pending!)}>{t('同じ適用操作を再確認', 'Retry the same application')}</button>{/if}
  <fieldset disabled={locked}><legend>{t('現在の人物を保存', 'Save a character')}</legend>
    <label>{t('保存する設定', 'Settings to save')}<select bind:value={kind}><option value="character">{t('人物（声・字幕・立ち絵）', 'Character (voice, subtitles, standing)')}</option><option value="voice">{t('声のみ', 'Voice only')}</option></select></label>
    <label>{t('人物', 'Character')}<select bind:value={characterId} onchange={() => { clipId = ''; name = project?.characters.find(c => c.id === characterId)?.name ?? ''; }}><option value="">{t('選択してください', 'Select')}</option>{#each project?.characters ?? [] as c}<option value={c.id}>{c.name}</option>{/each}</select></label>
    {#if kind === 'character'}<label>{t('立ち絵', 'Standing')}<select bind:value={clipId}><option value="">{t('声と字幕のみ', 'Voice and subtitles only')}</option>{#each project?.clips.filter(c => isStandingFor(c, characterId)) ?? [] as c}<option value={c.id}>{c.content.type} · {(c.start_us / 1e6).toFixed(2)}s</option>{/each}</select></label>{/if}
    <label>{t('保存名', 'Preset name')}<input bind:value={name} maxlength="1024" /></label>
    <button disabled={!characterId || !name.trim()} onclick={() => save()}>{t('新しいプリセットとして保存', 'Save new preset')}</button>
  </fieldset>
  <fieldset disabled={locked}><legend>{t('保存したプリセット', 'Saved presets')}</legend>
    <button onclick={() => request({ action: 'list', after_id: null, limit: 20 })}>{t('一覧を更新', 'Refresh')}</button>
    {#each items as item}<button class:chosen={selected?.id === item.id} onclick={() => { deleteConfirm = false; updateConfirm = false; void request({ action: 'get', id: item.id }); }}>{item.name} · {item.kind}</button>{/each}
    {#if next}<button onclick={() => request({ action: 'list', after_id: next, limit: 20 }, true)}>{t('続きを表示', 'Load more')}</button>{/if}
    {#if selected?.preset}
      <h3>{selected.preset.name}</h3>
      {#if selected.preset.content.kind === 'character'}
        <p>{selected.preset.content.character.credit}</p>
        {#if updateConfirm}<p>{t('上で選んだ人物・立ち絵・保存名で置き換えます。既存の作品は変わりません。', 'Replace with the character, standing and name selected above. Existing projects stay unchanged.')}</p><button disabled={kind !== 'character' || !characterId || !name.trim()} onclick={() => save(true)}>{t('更新を確定', 'Confirm update')}</button><button onclick={() => updateConfirm = false}>{t('取消', 'Cancel')}</button>{:else}<button disabled={kind !== 'character' || !characterId || !name.trim()} onclick={() => updateConfirm = true}>{t('選択中のプリセットを更新', 'Update selected preset')}</button>{/if}
        {#if selected.preset.content.standing}
          <label>{t('配置するトラック', 'Destination track')}<select bind:value={trackId}><option value="">{t('新しい立ち絵トラック', 'New standing track')}</option>{#each tracks as track}<option value={track.id}>{track.name}</option>{/each}</select></label>
          <label>{t('配置する側', 'Position')}<select bind:value={side}><option value="left">{t('左', 'Left')}</option><option value="right">{t('右', 'Right')}</option></select></label>
          <label>{t('表示時間（秒）', 'Duration (seconds)')}<input type="number" min="0.1" max="3600" step="0.1" bind:value={seconds} /></label>
          <p>{t('現在の再生位置へ追加します。新しいトラックも一度のUndoで戻せます。', 'Adds at the playhead. A new track is included in the same undo operation.')}</p>
        {/if}
        <button disabled={!!selected.preset.content.standing && ((!!trackId && !tracks.some(v => v.id === trackId)) || !Number.isFinite(seconds) || seconds <= 0)} onclick={apply}>{t('この作品へ人物を追加', 'Add character to this project')}</button>
      {:else if selected.preset.content.kind === 'voice'}
        {#key `${selected.id}:${selected.revision}`}<ApplyVoicePreset {editor} record={selected} {locked} onapply={() => { notice = ''; error = ''; }} />{/key}
        {#if updateConfirm}<p>{t('上で選んだ声と保存名で更新します。', 'Replace with the voice and name selected above.')}</p><button disabled={kind !== 'voice' || !characterId || !name.trim()} onclick={() => save(true)}>{t('更新を確定', 'Confirm update')}</button><button onclick={() => updateConfirm = false}>{t('取消', 'Cancel')}</button>{:else}<button disabled={kind !== 'voice' || !characterId || !name.trim()} onclick={() => updateConfirm = true}>{t('選択中のプリセットを更新', 'Update selected preset')}</button>{/if}
      {:else}<p>{t('制作テンプレートは「一括制作」→「制作テンプレートを保存・読み込む」から使用できます。', 'Use production templates under Batch production → Save or load production templates.')}</p>{/if}
      {#if deleteConfirm}<button onclick={() => { deleteConfirm = false; updateConfirm = false; void request({ action: 'delete', id: selected!.id, expected_revision: selected!.revision }); }}>{t('削除を確定', 'Confirm deletion')}</button>{:else}<button onclick={() => deleteConfirm = true}>{t('プリセットを削除', 'Delete preset')}</button>{/if}
    {/if}
  </fieldset>
</dialog>
<style>
  dialog { width:min(680px,90vw); max-height:85vh; overflow:auto; }
  header { position:sticky; top:-20px; background:var(--surface); z-index:1; padding:8px 0; display:flex; align-items:center; justify-content:space-between; gap:12px; }
  fieldset { margin:16px 0; padding:16px; border:1px solid var(--border); border-radius:8px; }
  label { display:grid; gap:4px; margin:10px 0; }
  button { margin:4px; } .chosen { outline:2px solid currentColor; }
</style>
