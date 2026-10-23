<script lang="ts">
  import { uiText } from '../../shared/i18n/extended-ui';
  import type { PsdPose, PsdProbe } from '@sungeki/api-client';
  import { psdOptions } from './psd-options';
  import { selectPoseOption } from './pose-draft';
  let { probe, pose = $bindable() }: { probe: PsdProbe; pose: PsdPose } = $props();
  const groups = $derived(psdOptions(probe, 1));
</script>
<details class="pose-settings">
  <summary>{uiText("表情・ポーズ")}</summary>
  <p>{uiText("「変更を適用」でプレビューに反映します。親の選択を変えると、その下の個別指定は元PSDの状態に戻ります。")}</p>
  {#each groups as group}
    <label>{group.label}<select value={pose.selections.find(selection => selection.group === group.path)?.option ?? ''}
      onchange={event => { pose = selectPoseOption(pose, group.path, event.currentTarget.value); }}>
      <option value="">{uiText("元PSDの表示状態")}</option>
      {#each group.options as option}<option value={option.path}>{option.label}</option>{/each}
    </select></label>
  {:else}
    <p>{uiText("この素材には選択可能なポーズのグループがありません。")}</p>
  {/each}
  <p>{uiText("表示される親グループの中から選んでください。口パク有効時の口形は「立ち絵の口パク」の設定が優先されます。")}</p>
</details>
<style>
  .pose-settings { border:1px solid var(--panel-border); border-radius:5px; padding:10px; margin-bottom:12px; background:var(--background); }
  summary { cursor:pointer; font-size:13px; font-weight:600; }
  label { display:flex; flex-direction:column; gap:4px; margin-bottom:10px; font-size:13px; }
  select { width:100%; min-width:0; }
  p { font-size:12px; line-height:1.6; }
</style>
