<script lang="ts">
  import { uiText } from '../../shared/i18n/extended-ui';
  import type { Character, PsdProbe, PsdSpeechBinding } from '@sungeki/api-client';
  import { psdOptions } from './psd-options';
  let { probe, characters, speech = $bindable() }: {
    probe: PsdProbe; characters: Character[]; speech: PsdSpeechBinding | null;
  } = $props();
  const groups = $derived(psdOptions(probe));
  const options = $derived(groups.find(group => group.path === speech?.group)?.options ?? []);
  let previous = $state<PsdSpeechBinding | null>(null);
  function enable(enabled: boolean) {
    if (enabled) speech = previous ?? { character_id: '', group: '', closed_option: '', open_option: '' };
    else { previous = speech ? { ...speech } : null; speech = null; }
  }
  function groupChanged(group: string) {
    if (speech) speech = { ...speech, group, closed_option: '', open_option: '' };
  }
  function closedChanged(closed_option: string) {
    if (speech) speech = { ...speech, closed_option, open_option: speech.open_option === closed_option ? '' : speech.open_option };
  }
</script>
<section aria-label={uiText("立ち絵の口パク")} class="standing-settings">
  <h4>{uiText("立ち絵の口パク")}</h4>
  <label class="toggle"><input type="checkbox" checked={speech !== null}
    disabled={!speech && (!characters.length || !groups.length)}
    onchange={event => enable(event.currentTarget.checked)} />{uiText("発話に合わせて口を動かす")}</label>
  {#if !characters.length}<p>{uiText("先にキャラクターを登録してください。")}</p>{/if}
  {#if !groups.length}<p>{uiText("この素材には開口・閉口を選べるグループがありません。")}</p>{/if}
  {#if speech}
    <label>{uiText("発話するキャラクター")}<select required bind:value={speech.character_id}>
      <option value="" disabled>{uiText("キャラクターを選択")}</option>
      {#each characters as character}<option value={character.id}>{character.name}</option>{/each}
    </select></label>
    <label>{uiText("口のグループ")}<select required value={speech.group} onchange={event => groupChanged(event.currentTarget.value)}>
      <option value="" disabled>{uiText("口のレイヤーがあるグループを選択")}</option>
      {#each groups as group}<option value={group.path}>{group.label}</option>{/each}
    </select></label>
    <label>{uiText("閉じた口")}<select required value={speech.closed_option} onchange={event => closedChanged(event.currentTarget.value)} disabled={!speech.group}>
      <option value="" disabled>{uiText("閉口の形を選択")}</option>
      {#each options as option}<option value={option.path}>{option.label}</option>{/each}
    </select></label>
    <label>{uiText("開いた口")}<select required bind:value={speech.open_option} disabled={!speech.group}>
      <option value="" disabled>{uiText("開口の形を選択")}</option>
      {#each options as option}<option value={option.path} disabled={option.path === speech.closed_option}>{option.label}</option>{/each}
    </select></label>
    <p>{uiText("同じキャラクターの発話に連動します。発話外とミュート中は閉じた口になります。")}</p>
    <p>{uiText("現在のポーズで表示されるグループを選んでください。音声に口パク解析がなければ、再合成が必要です。")}</p>
  {:else}
    <p>{uiText("口パクを使わず、保存されたポーズで表示します。")}</p>
  {/if}
  <p class="notice">{uiText("「変更を適用」で作品へ反映すると、プレビューの表示時刻に合わせて口形を確認できます。正式な動画出力にも反映されます。")}</p>
</section>
<style>
  .standing-settings { border:1px solid var(--panel-border); border-radius:5px; padding:10px; margin-bottom:12px; background:var(--background); }
  h4 { margin:0 0 12px; }
  label { display:flex; flex-direction:column; gap:4px; margin-bottom:10px; font-size:13px; }
  .toggle { flex-direction:row; align-items:center; }
  select { width:100%; min-width:0; }
  p { font-size:12px; line-height:1.6; }
  .notice { border-top:1px solid var(--panel-border); margin-bottom:0; padding-top:8px; color:var(--muted); }
</style>
