// Local, opt-in diagnostic trace. Record background events too: a hidden window
// is precisely one of the conditions being investigated, not a valid fps sample.
const events = ['play', 'playing', 'pause', 'ended', 'seeking', 'seeked',
  'waiting', 'stalled', 'error', 'timeupdate'] as const;
type Media = EventTarget & Pick<HTMLMediaElement,
  'currentTime' | 'paused' | 'ended' | 'seeking' | 'readyState' | 'networkState'>;

export function playbackEvents(media: Media, editorTime: () => number,
  visibility: () => string, now = () => performance.now()) {
  const rows: object[] = [];
  const started = now();
  let total = 0, stopped = false;
  const record = (event: string) => {
    rows.push({ event, elapsedMs: now() - started, mediaTime: media.currentTime,
      editorTime: editorTime(), paused: media.paused, ended: media.ended,
      seeking: media.seeking, readyState: media.readyState, networkState: media.networkState,
      visibility: visibility() });
    total++;
    if (rows.length > 256) rows.shift();
  };
  const listener = (event: Event) => record(event.type);
  for (const event of events) media.addEventListener(event, listener);
  record('diagnostic_start');
  return {
    stop() {
      if (stopped) return;
      stopped = true;
      for (const event of events) media.removeEventListener(event, listener);
      record('diagnostic_stop');
    },
    report: () => ({ total, retained: rows.length, limit: 256, events: [...rows],
      scope: 'Event-time observations including hidden windows; not physical A/V latency' }),
  };
}
