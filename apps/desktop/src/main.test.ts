import { afterEach, expect, test, vi } from 'vitest';

function page() {
  const status = { remove: vi.fn(), setAttribute: vi.fn(), replaceChildren: vi.fn() };
  const target = {};
  const events: string[] = [];
  vi.stubGlobal('document', {
    documentElement: { lang: '' },
    dispatchEvent: (event: Event) => { events.push(event.type); return true; },
    getElementById: (id: string) => id === 'app' ? target : status,
    createElement: () => ({ textContent: '', style: {} }),
  });
  vi.spyOn(console, 'error').mockImplementation(() => {});
  return { status, target, events };
}
afterEach(() => {
  vi.doUnmock('./app/bootstrap');
  vi.resetModules();
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

test('editor import failure leaves a visible error, with details as plain text', async () => {
  const { status, events } = page();
  vi.doMock('./app/bootstrap', () => { throw new Error('<script>load failed</script>'); });
  await import('./main');
  await vi.dynamicImportSettled();
  expect(events).toEqual(['next-startup-entry', 'next-startup-failed']);
  expect(status.remove).not.toHaveBeenCalled();
  expect(status.setAttribute).toHaveBeenCalledWith('role', 'alert');
  const nodes = status.replaceChildren.mock.calls[0];
  expect(nodes[0].textContent).toContain('起動できません');
  expect(nodes[2].textContent).toBeTruthy();
  expect(nodes[2]).not.toHaveProperty('innerHTML');
});

test('mount failure is reported instead of removing the startup notice', async () => {
  const { status, events } = page();
  vi.doMock('./app/bootstrap', () => ({ start: () => { throw new Error('mount failed'); } }));
  await import('./main');
  await vi.dynamicImportSettled();
  expect(events).toEqual(['next-startup-entry', 'next-startup-failed']);
  expect(status.remove).not.toHaveBeenCalled();
  expect(status.replaceChildren.mock.calls[0][2].textContent).toBe('mount failed');
});

test('successful mount removes only the startup notice', async () => {
  const { status, target, events } = page();
  const start = vi.fn();
  vi.doMock('./app/bootstrap', () => ({ start }));
  await import('./main');
  await vi.dynamicImportSettled();
  expect(events).toEqual(['next-startup-entry', 'next-startup-mounted']);
  expect(start).toHaveBeenCalledWith(target);
  expect(status.remove).toHaveBeenCalledOnce();
  expect(status.replaceChildren).not.toHaveBeenCalled();
});

test('startup failure follows the saved English preference without translating error details', async () => {
  const { status } = page();
  Object.assign(status, { querySelectorAll: () => [{ textContent: '' }, { textContent: '' }] });
  vi.stubGlobal('localStorage', { getItem: () => 'en' });
  vi.doMock('./app/bootstrap', () => ({ start: () => { throw new Error('原文の診断'); } }));
  await import('./main');
  await vi.dynamicImportSettled();
  const nodes = status.replaceChildren.mock.calls[0];
  expect(nodes[0].textContent).toBe('Unable to start');
  expect(nodes[1].textContent).toContain('Close and reopen');
  expect(nodes[2].textContent).toContain('原文の診断');
});
