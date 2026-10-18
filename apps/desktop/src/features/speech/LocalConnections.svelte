<script lang="ts">
  import { localEndpoint, saveLocalEndpoint } from './local-connection';
  let aivis = $state(localEndpoint('aivis_speech')), coeiroink = $state(localEndpoint('coeiroink'));
</script>
<label>AivisSpeech URL<input type="url" bind:value={aivis} onchange={() => saveLocalEndpoint('aivis_speech', aivis)} /></label>
<label>COEIROINK URL<input type="url" bind:value={coeiroink} onchange={() => saveLocalEndpoint('coeiroink', coeiroink)} /></label>
<style>label { display:flex; flex-direction:column; gap:5px; margin:10px 0; }</style>
