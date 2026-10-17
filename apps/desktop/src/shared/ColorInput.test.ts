import { expect, test } from 'vitest';
import { render } from 'svelte/server';
import ColorInput from './ColorInput.svelte';

test('rendered color validation accepts six hex digits and rejects invalid colors', () => {
  const { body } = render(ColorInput, { props: { label: '文字色', value: '#245ac4' } });
  const pattern = body.match(/pattern="([^"]+)"/)?.[1];
  expect(pattern).toBeDefined();
  const validator = new RegExp(`^(?:${pattern})$`, 'v');
  for (const value of ['#245ac4', '#ABCDEF', '#000000']) expect(validator.test(value)).toBe(true);
  for (const value of ['red', '#123', '#12345678', '#12gg56']) expect(validator.test(value)).toBe(false);
  expect(body).toContain('type="color"');
});
