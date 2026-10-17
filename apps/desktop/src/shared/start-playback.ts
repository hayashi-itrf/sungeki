// pause() can reject a pending play() with AbortError. This is expected when
// toggling quickly, switching players, or removing a preview, not a decode failure.
export async function startPlayback(media: Pick<HTMLMediaElement, 'play' | 'error'>,
  onFailure: () => void): Promise<void> {
  try { await media.play(); }
  catch (error) {
    if (error instanceof DOMException && error.name === 'AbortError' && !media.error) return;
    onFailure();
  }
}
