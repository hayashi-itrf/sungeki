<script lang="ts">
  import type { Clip } from '@sungeki/api-client';
  import type { EditorController } from '../editor/controller.svelte';
  import { commitPose } from './commit-pose';
  import VrmPoseFields from './VrmPoseFields.svelte';
  let { editor, clip }: { editor: EditorController; clip: Clip } = $props();
  let opened = $state(false);
  function begin() {
    opened = true; editor.drafting = true;
    return () => { opened = false; editor.acceptIncoming(); };
  }

</script>
{#if clip.content.type === 'vrm' && editor.snapshot}
  {#if editor.drafting && !opened}<p>他の欄の入力を適用するか取り消すと、ポーズを編集できます。</p>{/if}
  <VrmPoseFields value={clip.content.standing} assets={editor.snapshot.project.assets}
    sessionId={editor.snapshot.session_id} revision={editor.snapshot.revision}
    disabled={!opened && (editor.drafting || editor.busy || !!editor.pending || !!editor.incoming)}
    blocked={!!editor.pending || !!editor.incoming} error={editor.error}
    durationUs={clip.duration_us} initialTimeUs={Math.max(0, Math.round(editor.time * 1e6) - clip.start_us)}
    beginPose={begin} commit={pose => commitPose(editor, clip, pose)} />
{/if}
