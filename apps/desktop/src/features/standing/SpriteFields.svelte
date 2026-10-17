<script lang="ts">
  import { locale } from '../../shared/i18n/locale.svelte';
  import { addPose, renamePose, removePose } from './sprite-draft';
  import type { Asset, Character, ClipContent } from '@sungeki/api-client';
  type Sprite = Extract<ClipContent, { type: 'sprite' }>;
  let { value = $bindable(), assets, characters, adding = false, changed = () => {} }: {
    value: Sprite; assets: Asset[]; characters: Character[]; adding?: boolean; changed?: () => void;
  } = $props();
  let images = $derived(assets.filter(a => a.probe.type === 'image' && a.probe.info.format === 'png' && a.probe.info.orientation === 1));
  let selected = $derived(value.sprites.poses.find(p => p.name === (value.pose ?? value.sprites.default_pose)));
  let error = $state('');
  // Keep browser validity attached to the same row until every name is valid.
  let nameErrors = $state<Record<number, string>>({});
  let invalidNames = $derived(Object.keys(nameErrors).length > 0);
  let posesOpen = $state((() => adding)());
  function rename(event: Event, index: number) {
    const input = event.currentTarget as HTMLInputElement;
    try { value = renamePose(value, index, input.value); input.setCustomValidity(''); delete nameErrors[index]; error = ''; changed(); }
    catch (cause) {
      nameErrors[index] = String(cause instanceof Error ? cause.message : cause);
      input.setCustomValidity(nameErrors[index]);
    }
  }
  function update(action: () => Sprite) {
    if (invalidNames) return;
    try { value = action(); error = ''; changed(); }
    catch (cause) { error = String(cause instanceof Error ? cause.message : cause); }
  }
</script>
<section aria-label={locale.text('spriteSettings')}>
  <h4>{locale.text('spriteStanding')}</h4>
  <label>{locale.text('spriteShownPose')}<select bind:value={value.pose}>
    <option value={null}>{locale.text('spriteDefaultOption').replace('{name}', () => value.sprites.default_pose)}</option>
    {#each value.sprites.poses as pose}<option value={pose.name}>{pose.name}</option>{/each}
  </select></label>
  <label>{locale.text('standingCharacter')}<select bind:value={value.character_id}>
    <option value={null}>{locale.text('spriteStatic')}</option>
    {#each characters as character}<option value={character.id}>{character.name}</option>{/each}
  </select></label>
  {#if characters.length === 0}<p>{locale.text('spriteRegisterCharacter')}</p>{/if}
  {#if !selected?.open_asset_id}<p>{locale.text('spriteNoOpen')}</p>{/if}
  <p>{locale.text('spriteLipHint')}</p>
  <details bind:open={posesOpen}>
    <summary>{locale.text('spriteManage')}</summary>
    {#if error}<p role="alert">{error}</p>{/if}
    <label>{locale.text('spriteDefaultPose')}<select bind:value={value.sprites.default_pose}>{#each value.sprites.poses as pose}<option value={pose.name}>{pose.name}</option>{/each}</select></label>
    <p>{locale.text('spriteDimensionsHint')}</p>
    {#each value.sprites.poses as pose, index}
      <fieldset>
        <legend>{pose.name}</legend>
        {#if nameErrors[index]}<p role="alert">{nameErrors[index]}</p>{/if}
        <label>{locale.text('spritePoseName')}<input required aria-invalid={!!nameErrors[index]} value={pose.name} oninput={event => rename(event, index)} /></label>
        <label>{locale.text('spriteClosedLabel').replace('{name}', () => pose.name)}<select required bind:value={pose.closed_asset_id}>
          {#each images as image}<option value={image.id}>{image.name}</option>{/each}
        </select></label>
        <label>{locale.text('spriteOpenLabel').replace('{name}', () => pose.name)}<select bind:value={pose.open_asset_id}>
          <option value={null}>{locale.text('spriteUnused')}</option>
          {#each images as image}<option value={image.id}>{image.name}</option>{/each}
        </select></label>
        <button type="button" disabled={invalidNames || value.sprites.poses.length <= 1 || pose.name === value.sprites.default_pose || pose.name === value.pose} onclick={() => update(() => removePose(value, index))}>{locale.text('spriteDeletePose')}</button>
      </fieldset>
    {/each}
    <button type="button" disabled={invalidNames || value.sprites.poses.length >= 256} onclick={() => update(() => addPose(value))}>{locale.text('spriteDuplicatePose')}</button>
    <p>{locale.text('spriteDeleteHint')}</p>
  </details>
  <p>{adding ? locale.text('spriteAddHint') : locale.text('spriteApplyHint')}{locale.text('spriteRenderHint')}</p>
</section>
<style>
  section { border:1px solid var(--panel-border); border-left:3px solid var(--accent); border-radius:5px; padding:10px; margin-bottom:12px; background:var(--panel-header); }
  h4 { margin:0 0 10px; } p { font-size:11px; line-height:1.5; }
  label { display:flex; flex-direction:column; gap:4px; font-size:12px; margin-bottom:10px; }
  select { width:100%; box-sizing:border-box; } fieldset { margin-top:10px; border:1px solid var(--panel-border); }
  summary { cursor:pointer; font-size:12px; }
</style>
