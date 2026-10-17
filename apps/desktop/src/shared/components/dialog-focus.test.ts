import { afterEach, expect, test, vi } from 'vitest';
import { containDialogFocus } from './dialog-focus';

// DOM facade reproduces the native observation: closed details descendants can
// still report rectangles. This checks the handler, not actual WebKit traversal.
function fixture() {
  let handler: ((event: KeyboardEvent) => void) | undefined;
  const document = { activeElement: null as ElementStub | null };
  class ElementStub {
    parentElement: ElementStub | null = null;
    children: ElementStub[] = [];
    tabIndex = 0;
    explicitTabIndex = false;
    open = true;
    disabled = false;
    inert = false;
    visible = true;
    rectangles = true;
    ownerDocument = document;
    constructor(readonly tag: string) {}
    append(tag: string) {
      const child = new ElementStub(tag);
      child.parentElement = this;
      this.children.push(child);
      return child;
    }
    matches(selector: string) {
      return selector === 'summary' ? this.tag === 'summary'
        : selector === ':disabled' ? this.disabled
          : selector === 'details:not([open])' && this.tag === 'details' && !this.open;
    }
    hasAttribute(name: string) { return name === 'tabindex' && this.explicitTabIndex; }
    contains(element: ElementStub | null): boolean {
      return element === this || this.children.some((child) => child.contains(element));
    }
    closest(): ElementStub | null { return this.inert ? this : this.parentElement?.closest() ?? null; }
    querySelector() { return this.children.find((child) => child.tag === 'summary') ?? null; }
    querySelectorAll(): ElementStub[] {
      return this.children.flatMap((child) => [
        ...(['button', 'summary', 'input'].includes(child.tag) ? [child] : []), ...child.querySelectorAll(),
      ]);
    }
    getClientRects() { return this.rectangles ? [{}] : []; }
    focus() { document.activeElement = this; }
    addEventListener(_name: string, listener: (event: KeyboardEvent) => void) { handler = listener; }
    removeEventListener(_name: string, listener: (event: KeyboardEvent) => void) {
      if (handler === listener) handler = undefined;
    }
  }
  vi.stubGlobal('getComputedStyle', (element: ElementStub) => ({ visibility: element.visible ? 'visible' : 'hidden' }));
  const dialog = new ElementStub('dialog');
  const close = dialog.append('button');
  const details = dialog.append('details');
  details.open = false;
  const summary = details.append('summary');
  summary.tabIndex = -1; // Native implicit summary values must not exclude it.
  const input = details.append('input');
  close.focus();
  const action = containDialogFocus(dialog as unknown as HTMLDialogElement);
  function press(overrides: Partial<KeyboardEvent> = {}) {
    const event = { key: 'Tab', defaultPrevented: false, shiftKey: false,
      altKey: false, ctrlKey: false, metaKey: false, ...overrides, preventDefault: vi.fn() };
    handler?.(event as unknown as KeyboardEvent);
    return event.preventDefault;
  }
  return { dialog, close, details, summary, input, document, action, press };
}
afterEach(() => vi.unstubAllGlobals());

test('collapsed details never send reverse Tab to an invisible descendant with rectangles', () => {
  const f = fixture();
  expect(f.press({ shiftKey: true })).toHaveBeenCalledOnce();
  expect(f.document.activeElement).toBe(f.summary);
  expect(f.press()).toHaveBeenCalledOnce();
  expect(f.document.activeElement).toBe(f.close);
});

test('opening and closing sections updates the boundary without reinstalling the action', () => {
  const f = fixture();
  f.details.open = true;
  f.press({ shiftKey: true });
  expect(f.document.activeElement).toBe(f.input);
  f.press();
  expect(f.document.activeElement).toBe(f.close);
  f.details.open = false;
  f.press({ shiftKey: true });
  expect(f.document.activeElement).toBe(f.summary);
});

test('an open nested section cannot expose controls through a closed ancestor', () => {
  const f = fixture();
  const nested = f.details.append('details');
  nested.append('summary');
  nested.append('button');
  f.press({ shiftKey: true });
  expect(f.document.activeElement).toBe(f.summary);
});

test('disabled, inert, hidden and explicitly excluded controls are not boundaries', () => {
  const f = fixture();
  f.details.open = true;
  f.dialog.append('button').disabled = true;
  f.dialog.append('button').inert = true;
  f.dialog.append('button').visible = false;
  f.dialog.append('button').rectangles = false;
  const excluded = f.dialog.append('summary');
  excluded.tabIndex = -1;
  excluded.explicitTabIndex = true;
  f.press({ shiftKey: true });
  expect(f.document.activeElement).toBe(f.input);
});

test('interior traversal, Escape and handled or modified keys retain native behavior', () => {
  const f = fixture();
  f.details.open = true;
  f.summary.focus();
  expect(f.press()).not.toHaveBeenCalled();
  f.close.focus();
  for (const overrides of [{ key: 'Escape' }, { defaultPrevented: true }, { altKey: true }, { ctrlKey: true }, { metaKey: true }]) {
    expect(f.press({ shiftKey: true, ...overrides })).not.toHaveBeenCalled();
  }
  f.dialog.open = false;
  expect(f.press({ shiftKey: true })).not.toHaveBeenCalled();
});

test('empty dialogs retain focus, and removing the action removes its handler', () => {
  const f = fixture();
  f.dialog.children = [];
  f.document.activeElement = null;
  expect(f.press()).toHaveBeenCalledOnce();
  expect(f.document.activeElement).toBe(f.dialog);
  f.action.destroy();
  expect(f.press()).not.toHaveBeenCalled();
});
