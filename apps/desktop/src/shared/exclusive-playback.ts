// A new preview pauses other players without losing their review positions.
// This action also stops media when its dialog, result or session is removed.
export function exclusivePlayback(node: HTMLMediaElement) {
  const document = node.ownerDocument;
  const interrupt = (event: Event) => {
    if (event.target instanceof HTMLMediaElement && event.target !== node) node.pause();
  };
  const pause = () => node.pause();
  document.addEventListener('vvm-editing-play', pause);
  const options = { capture: true };
  document.addEventListener('play', interrupt, options);
  return {
    destroy() {
      document.removeEventListener('play', interrupt, options);
      document.removeEventListener('vvm-editing-play', pause);
      node.pause();
    },
  };
}
