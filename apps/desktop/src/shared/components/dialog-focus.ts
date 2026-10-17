/** Keep keyboard traversal inside the active modal, including in native WebKit. */
export function containDialogFocus(dialog: HTMLDialogElement) {
  function insideOpenDetails(element: HTMLElement) {
    for (let parent = element.parentElement; parent && parent !== dialog; parent = parent.parentElement) {
      if (parent.matches('details:not([open])')
        && !parent.querySelector(':scope > summary')?.contains(element)) return false;
    }
    return true;
  }
  function keydown(event: KeyboardEvent) {
    if (event.defaultPrevented || event.key !== 'Tab' || event.altKey || event.ctrlKey || event.metaKey || !dialog.open) return;
    const controls = [...dialog.querySelectorAll<HTMLElement>(
      'button, input, select, textarea, a[href], summary, [tabindex], [contenteditable="true"]',
    // Include native summary controls even when their implicit tabIndex is negative.
    )].filter((element) => (element.tabIndex >= 0
      || (element.matches('summary') && !element.hasAttribute('tabindex')))
      && !element.matches(':disabled')
      && !element.closest('[inert]') && insideOpenDetails(element) && element.getClientRects().length > 0
      && getComputedStyle(element).visibility !== 'hidden');
    const active = dialog.ownerDocument.activeElement;
    const destination = event.shiftKey ? controls.at(-1) : controls[0];
    const boundary = event.shiftKey ? controls[0] : controls.at(-1);
    if (!controls.length || active === dialog || active === boundary || !dialog.contains(active)) {
      event.preventDefault();
      (destination ?? dialog).focus();
    }
  }
  dialog.addEventListener('keydown', keydown);
  return { destroy: () => dialog.removeEventListener('keydown', keydown) };
}
