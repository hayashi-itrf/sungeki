<script lang="ts">
  import { locale } from '../../shared/i18n/locale.svelte';
  import type { MessageKey } from '../../shared/i18n/messages';
  import { open } from '@tauri-apps/plugin-dialog';
  import type { RuntimeSettings } from '@sungeki/api-client';
  import { hostApi } from '../../shared/host-api';
  import { runtimeDiagnostic } from './runtime-diagnostic';
  let saved = $state<RuntimeSettings | null>(null);
  let draft = $state<RuntimeSettings | null>(null);
  const modified = $derived(saved !== null && draft !== null && JSON.stringify(saved) !== JSON.stringify(draft));
  let busy = $state(false);
  let messageKey = $state<MessageKey>('runtimeLoadHint');
  let errorMessage = $state('');
  const fields: { key: Exclude<keyof RuntimeSettings, 'format_version'>; label: MessageKey }[] = [
    { key: 'voicevox_engine', label: 'runtimeVoicevoxEngine' },
    { key: 'aivis_engine', label: 'runtimeAivisEngine' },
    { key: 'coeiroink_engine', label: 'runtimeCoeiroinkEngine' },
    { key: 'runtime_root', label: 'runtimeStorage' },
    { key: 'python', label: 'runtimePython' },
    { key: 'image_worker', label: 'runtimeImageWorker' },
    { key: 'ffmpeg', label: 'runtimeFfmpeg' },
    { key: 'ffprobe', label: 'runtimeFfprobe' },
    { key: 'font', label: 'runtimeFont' },
    { key: 'font_sha256', label: 'runtimeFontHash' },
    { key: 'vrm_node', label: 'runtimeNode' },
    { key: 'vrm_render_worker', label: 'runtimeVrmRender' },
    { key: 'vrm_inspect_worker', label: 'runtimeVrmInspect' },
  ];
  async function chooseFile(field: typeof fields[number]) {
    if (field.key === 'font_sha256' || busy || !draft) return;
    busy = true; errorMessage = '';
    try {
      // Native file pickers may resolve the venv's Python symlink to the base
      // interpreter, losing its installed packages. Select the venv root instead.
      const path = await open({ title: locale.text(field.key === 'python' ? 'runtimeChoosePythonEnvironment' : field.label), multiple: false, directory: field.key === 'python' || field.key === 'runtime_root' });
      if (typeof path === 'string' && draft) {
        draft[field.key] = field.key === 'python' ? `${path.replace(/\/$/, '')}/bin/python` : path;
        if (field.key === 'font') draft.font_sha256 = null;
      }
    } catch (error) { errorMessage = error instanceof Error ? error.message : String(error); }
    finally { busy = false; }
  }
  async function run(save: boolean) {
    busy = true; errorMessage = '';
    try {
      const response = await hostApi.session(save && saved && draft
        ? { type: 'save_runtime_settings', args: { expected: saved, settings: draft } }
        : { type: 'get_runtime_settings' });
      if (response.type !== 'runtime_settings') throw new Error(locale.text('runtimeInvalidResponse'));
      saved = structuredClone(response.settings);
      draft = structuredClone(response.settings);
      messageKey = save ? 'runtimeSaved' : 'runtimeLoaded';
    } catch (error) {
      errorMessage = typeof error === 'object' && error !== null && 'message' in error ? String(error.message) : String(error);
    } finally { busy = false; }
  }
</script>
<details class="settings">
  <summary>{locale.text('runtimeTitle')}</summary>
  <p>{locale.text('runtimeScope')}</p>
  <section aria-label={locale.text('runtimeNeedsTitle')}>
    <h3>{locale.text('runtimeNeedsTitle')}</h3>
    <ul>
      <li>{locale.text('runtimeNeedsEditing')}</li>
      <li>{locale.text('runtimeNeedsAudio')}</li>
      <li>{locale.text('runtimeNeedsVideo')}</li>
      <li>{locale.text('runtimeNeedsVrm')}</li>
      <li>{locale.text('runtimeNeedsSpeech')}</li>
    </ul>
  </section>
  <p>{locale.text('runtimeOverrides')}</p>
  <p>{locale.current === 'ja' ? 'VOICEVOX・AivisSpeech・COEIROINKの実行ファイルを指定すると、話者取得・合成時に自動起動します。空欄なら手動起動です。Sungekiが起動したエンジンはホスト終了時に停止します。起動済みの他アプリは停止しません。変更は次回起動から反映します。' : 'Configure engine executables for automatic startup on voice discovery or synthesis. Leave blank for manual startup. Engines launched here stop when the host exits; external engines are never stopped. Changes apply on the next engine launch.'}</p>
  <button disabled={busy} onclick={() => run(false)}>{locale.text(modified ? 'runtimeDiscardAndLoad' : 'runtimeLoad')}</button>
  {#if draft}
    <fieldset disabled={busy}>
      <legend>{locale.text('runtimeLocations')}</legend>
      {#each fields as field}
        <label>{locale.text(field.label)}
          <input value={draft[field.key] ?? ''} oninput={(event) => {
            if (draft) {
              if (field.key === 'voicevox_engine' || field.key === 'aivis_engine' || field.key === 'coeiroink_engine') draft[field.key] = event.currentTarget.value || undefined;
              else draft[field.key] = event.currentTarget.value || null;
              if (field.key === 'font') draft.font_sha256 = null;
            }
          }} spellcheck="false" />
        </label>
        {#if field.key !== 'font_sha256'}
          <button class="choose" type="button" aria-label={`${locale.text(field.key === 'python' ? 'runtimeChoosePythonEnvironment' : field.key === 'runtime_root' ? 'runtimeChooseStorageFolder' : 'runtimeChooseToolFile')}: ${locale.text(field.label)}`} onclick={() => chooseFile(field)}>{locale.text(field.key === 'python' ? 'runtimeChoosePythonEnvironment' : field.key === 'runtime_root' ? 'runtimeChooseStorageFolder' : field.key === 'font' ? 'runtimeChooseFontFile' : 'runtimeChooseToolFile')}</button>
        {/if}
        {#if field.key === 'font'}<small>{locale.text('runtimeFontHint')}</small>{/if}
        {#if field.key === 'runtime_root'}<small>{locale.text('runtimeStorageHint')}</small>{/if}
        {#if field.key === 'python'}<small>{locale.text('runtimePythonHint')}</small>{/if}
      {/each}
    </fieldset>
    <button disabled={busy || !modified} onclick={() => run(true)}>{locale.text('runtimeSave')}</button>
  {/if}
  <p role="status">{errorMessage ? runtimeDiagnostic(errorMessage) : locale.text(modified ? 'runtimeUnsaved' : messageKey)}</p>
</details>
<style>
  fieldset { display: grid; gap: 10px; margin: 12px 0; border: 1px solid var(--border); }
  label { display: grid; gap: 4px; }
  input { width: 100%; box-sizing: border-box; }
  .choose { justify-self: start; }
  section { padding:12px; border:1px solid var(--panel-border); border-radius:6px; background:var(--panel-header); }
  h3 { margin:0 0 8px; font-size:14px; }
  ul { margin:0; padding-left:20px; }
  li + li { margin-top:8px; }
</style>
