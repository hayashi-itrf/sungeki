export function playbackShortcut(event: KeyboardEvent, toggle: () => void): void {
  if (event.code !== 'Space' || event.repeat || event.isComposing || event.defaultPrevented
    || event.altKey || event.ctrlKey || event.metaKey || event.shiftKey) return;
  const target = event.target;
  if (target instanceof Element && target.closest('input,textarea,select,button,a,[contenteditable]:not([contenteditable="false"]),[role="button"],[role="slider"],[role="textbox"],dialog,[role="dialog"]')) return;
  if (document.querySelector('dialog[open],[role="dialog"][aria-modal="true"]')) return;
  event.preventDefault(); toggle();
}
