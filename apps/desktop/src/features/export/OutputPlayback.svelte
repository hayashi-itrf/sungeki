<script lang="ts">
  import PlaybackVideo from '../editor/PlaybackVideo.svelte';
  let { jobId }: { jobId: string } = $props();
  // Reviewing an old output never moves the current document's playhead.
  const position = $state({ time: 0, seekGeneration: 0, previewVolume: 1 });
</script>
<div class="output-playback">
  <PlaybackVideo editor={position} {jobId} />
</div>
<style>
  .output-playback { display:flex; flex-direction:column; flex:1; min-height:0; gap:8px; }
</style>
