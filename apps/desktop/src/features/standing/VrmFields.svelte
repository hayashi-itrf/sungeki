<script lang="ts">
  import { locale } from '../../shared/i18n/locale.svelte';
  import { uiText } from '../../shared/i18n/extended-ui';
  import type { Asset, Character, VrmStanding } from '@sungeki/api-client';
  let { value = $bindable(), assets, characters, changed = () => {} }: {
    value: VrmStanding; assets: Asset[]; characters: Character[]; changed?: () => void;
  } = $props();
  let models = $derived(assets.filter(a => a.probe.type === 'vrm' && a.probe.info.inspection.kind === 'model'));
  let motions = $derived(assets.filter(a => a.probe.type === 'vrm' && a.probe.info.inspection.kind === 'motion'));
  let model = $derived(models.find(a => a.id === value.model_asset_id));
  let names = $derived(model?.probe.type === 'vrm' ? model.probe.info.inspection.expressions : []);
  let unsupported = $derived(Object.keys(value.expressions).filter(name => !names.includes(name)));
  function remove(name: string) {
    const expressions = { ...value.expressions }; delete expressions[name];
    value = { ...value, expressions }; changed();
  }
</script>
<section aria-label={uiText("VRMの設定")}>
  <h4>{uiText("VRM立ち絵")}</h4>
  <label>{uiText("モデル")}<select required bind:value={value.model_asset_id}>
    {#each models as asset}<option value={asset.id}>{asset.name}</option>{/each}
  </select></label>
  <label>{uiText("モーション")}<select required bind:value={value.motion_asset_id}>
    {#each motions as asset}<option value={asset.id}>{asset.name}</option>{/each}
  </select></label>
  <label>{uiText("モーションの開始位置（秒）")}<input type="number" required min="0" max="7200" step="0.000001"
    value={value.source_start_us / 1e6} oninput={e => { value.source_start_us = Math.round(e.currentTarget.valueAsNumber * 1e6); }} /></label>
  <p>{uiText("モーションは繰り返し再生します。クリップの開始時刻とは別の設定です。")}</p>
  <label>{uiText("口パクを合わせるキャラクター")}<select bind:value={value.character_id}>
    <option value={null}>{uiText("連動しない（指定した表情を使う）")}</option>
    {#each characters as character}<option value={character.id} disabled={!names.includes('aa')}>{character.name}</option>{/each}
  </select></label>
  {#if !names.includes('aa')}<p>{uiText("このモデルは「aa」表情がないため、口パク連動を使えません。")}</p>{/if}
  {#if value.character_id}<p>{uiText("音声解析に合わせて口を開閉します。発話外・ミュート中は閉口します。「aa」の指定値は口パクに置き換わります。")}</p>{/if}
  <details>
    <summary>{uiText("表情の強さ")}</summary>
    <p>{uiText("指定を解除した表情はモーション側の値を使います。")}</p>
    {#each names as name}
      <label>{name}<input type="number" min="0" max="1" step="0.05" required
        disabled={name === 'aa' && value.character_id !== null}
        value={value.expressions[name] ?? 0}
        oninput={e => { value.expressions = { ...value.expressions, [name]: e.currentTarget.valueAsNumber }; }} /></label>
      {#if name in value.expressions}<button type="button" onclick={() => remove(name)}>{locale.current === 'en' ? `Remove override: ${name}` : `「${name}」の指定を解除`}</button>{/if}
    {/each}
  </details>
  {#each unsupported as name}
    <p role="alert">{locale.current === 'en' ? `The saved expression ${name} is not available in this model.` : `このモデルにない表情「${name}」が残っています。`}</p>
    <button type="button" onclick={() => remove(name)}>{locale.current === 'en' ? `Remove override: ${name}` : `「${name}」の指定を解除`}</button>
  {/each}
  <p>{uiText("「変更を適用」で編集プレビューと出力に反映します。正式な画像を確認することもできます。")}</p>
</section>
<style>
  section { border:1px solid var(--panel-border); border-left:3px solid var(--accent); border-radius:5px; padding:10px; margin-bottom:12px; background:var(--panel-header); }
  h4 { margin:0 0 10px; } p { font-size:11px; line-height:1.5; }
  label { display:flex; flex-direction:column; gap:4px; font-size:12px; margin-bottom:10px; }
  select,input { width:100%; box-sizing:border-box; } button { margin-bottom:8px; }
</style>
