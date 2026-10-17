<script lang="ts">
  import { renderDiagnostic } from './render-diagnostic';
  let { message, actions = [] }: { message: string; actions?: readonly string[] } = $props();
</script>
{#if message}
  <div role="alert">
    <p>{renderDiagnostic(message)}</p>
    {#if actions.length}<ul>{#each actions as action}<li>{renderDiagnostic(action)}</li>{/each}</ul>{/if}
  </div>
{/if}
<style>
  div { padding:10px; border:1px solid var(--panel-border); border-left:4px solid var(--accent); border-radius:4px; overflow-wrap:anywhere; }
  p { margin:0; } ul { margin:8px 0 0; padding-left:22px; }
  li + li { margin-top:5px; }
</style>
