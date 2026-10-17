<script lang="ts">
  import { onDestroy } from 'svelte';
  import type { BuildVideoJob } from '@sungeki/api-client';
  import { hostApi } from '../../shared/host-api';
  import { locale } from '../../shared/i18n/locale.svelte';
  import RenderPlayer from '../export/RenderPlayer.svelte';
  import PublishOutput from '../export/PublishOutput.svelte';
  import { PublicationController } from '../export/publication.svelte';
  import { BuildPlayback } from './playback.svelte';
  let { job }: { job: BuildVideoJob } = $props();
  const playback = new BuildPlayback(hostApi);
  const publication = (() => new PublicationController(hostApi, job.session_id))();
  onDestroy(() => { playback.dispose(); publication.dispose(); });
</script>
<button disabled={playback.busy || publication.busy || !!publication.pending} onclick={() => playback.load(job)}>{playback.busy ? locale.text('buildPlaybackChecking') : playback.job ? locale.text('buildPlaybackRefresh') : locale.text('buildPlaybackOpen')}</button>
{#if playback.error}<p role="alert">{playback.error}</p>{/if}
{#if playback.job}
  {#if playback.job.stale}<p role="status">{locale.text('renderStale')}</p>{/if}
  <RenderPlayer job={playback.job} initiallyOpen={true} />
  <PublishOutput {publication} job={playback.job} disabled={playback.busy} />
{/if}
