// One display decode at a time. Disposed components check cancellation before entering the host.
let tail: Promise<unknown> = Promise.resolve();
export function queueImage<T>(work: () => Promise<T>): Promise<T> {
  const next = tail.then(work, work);
  tail = next.catch(() => undefined);
  return next;
}
