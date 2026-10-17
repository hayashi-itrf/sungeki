<script lang="ts">
  import { onDestroy, onMount, untrack } from 'svelte';
  import { locale } from '../../shared/i18n/locale.svelte';
  import { hostApi } from '../../shared/host-api';
  import type { EditorController } from './controller.svelte';
  import Preview from './Preview.svelte';
  import PlaybackTransport from './PlaybackTransport.svelte';
  import { LivePlayer } from './live-player';
  import { playbackShortcut } from './playback-shortcut';
  let { editor, proxies, generation, blocked }: { editor: EditorController; proxies: Readonly<Record<string, string>>; generation: number; blocked: boolean } = $props();
  let playing = $state(false), buffering = $state(false), error = $state('');
  const sessionId = untrack(() => editor.snapshot!.session_id);
  let published = 0;
  const player = (() => new LivePlayer(hostApi, editor.snapshot!, (time, active, loading) => {
    if (editor.snapshot?.session_id !== sessionId) return;
    published = time; editor.time = time; editor.seekGeneration++;
    playing = active; buffering = loading;
  }, cause => { error = cause && typeof cause === 'object' && 'message' in cause ? String(cause.message) : String(cause); }))();
  $effect(() => player.setVolume(editor.previewVolume));
  const displayTime = $derived(Math.min(editor.time, Math.max(0, player.duration - 1e-6)));
  untrack(() => player.seek(editor.time));
  function toggle() {
    if (playing) player.pause();
    else if (!blocked) { error = ''; void player.play(editor.time); }
  }
  $effect(() => { if (blocked) untrack(() => player.pause()); });
  $effect(() => { const time = editor.time; if (Math.abs(time - published) > 1e-6) untrack(() => player.seek(time)); });
  onMount(() => {
    const pause = () => player.pause();
    document.addEventListener('play', pause, true);
    return () => document.removeEventListener('play', pause, true);
  });
  onDestroy(() => player.dispose());
</script>
<svelte:window onkeydown={event => playbackShortcut(event, toggle)} onblur={() => player.pause()} />
<svelte:document onvisibilitychange={() => { if (document.hidden) player.pause(); }} />
<Preview {proxies} project={editor.snapshot!.project} time={displayTime} sessionId={editor.snapshot!.session_id} revision={editor.snapshot!.revision} {generation} />
<PlaybackTransport {playing} duration={player.duration} time={editor.time} volume={editor.previewVolume} disabled={blocked || !player.duration}
  ontoggle={toggle} onseek={time => player.seek(time)} onvolume={value => { editor.previewVolume = value; player.setVolume(value); }} />
<small>{locale.current === 'ja' ? 'Space：再生／一時停止。編集用の簡易再生です。完成映像と最終音量は確認用動画で確認できます。' : 'Space: play / pause. Editing preview; use a rendered preview to check final visuals and mastering.'}</small>
{#if buffering}<small role="status">{locale.current === 'ja' ? '音声を読み込んでいます…' : 'Loading audio…'}</small>{/if}
{#if player.plan.missing}<small role="status">{locale.current === 'ja' ? `音声未合成のセリフ：${player.plan.missing}件（字幕のみ再生）` : `${player.plan.missing} lines have no synthesized audio (subtitles only).`}</small>{/if}
{#if error}<p role="alert">{error}</p>{/if}
<style>small { color:var(--muted); font-size:11px; flex-shrink:0; } p { color:var(--danger); }</style>
