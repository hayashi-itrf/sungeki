import { afterEach, expect, test, vi } from 'vitest';
import { render } from 'svelte/server';
import type { EditorController } from '../editor/controller.svelte';
import { locale } from '../../shared/i18n/locale.svelte';
import QwenCharacterForm from './QwenCharacterForm.svelte';
import Sbv2CharacterForm from './Sbv2CharacterForm.svelte';
import IrodoriCharacterForm from './IrodoriCharacterForm.svelte';
import QwenProfilePicker from './QwenProfilePicker.svelte';
import { QwenProfiles } from './qwen-profiles.svelte';
import { qwenLanguageLabel } from './qwen-language';

afterEach(() => { locale.current = 'ja'; });

test('all managed character forms render translated controls and hints', () => {
  const editor = { busy: false, pending: null } as EditorController;
  for (const language of ['ja', 'en'] as const) {
    locale.current = language;
    for (const component of [QwenCharacterForm, Sbv2CharacterForm, IrodoriCharacterForm]) {
      const { body } = render(component, { props: { editor, submitting: vi.fn() } });
      for (const key of ['voiceProfileSelect', 'voiceProfileRefresh', 'voiceProfileHint',
        'voiceCharacterName', 'voiceCredit', 'voiceCreditHint', 'voiceRegister', 'voiceRegisterHint'] as const) {
        expect(body).toContain(locale.text(key));
      }
      if (language === 'en') expect(body).not.toMatch(/[ぁ-んァ-ン一-龯]/);
    }
  }
});

test('profile empty and loading states are translated without starting engines', () => {
  const session = vi.fn();
  const profiles = new QwenProfiles({ session });
  profiles.loaded = true;
  for (const language of ['ja', 'en'] as const) {
    locale.current = language;
    profiles.busy = false;
    expect(render(QwenProfilePicker, { props: { profiles } }).body).toContain(locale.text('voiceProfileEmpty'));
    profiles.busy = true;
    const { body } = render(QwenProfilePicker, { props: { profiles } });
    expect(body).toContain(locale.text('voiceProfileLoading'));
    expect(body).toContain('role="status"');
  }
  expect(session).not.toHaveBeenCalled();
});

test('localized language labels leave model values and unknown labels intact', () => {
  expect(qwenLanguageLabel('japanese', 'en')).toBe('Japanese');
  expect(qwenLanguageLabel('Japanese', 'ja')).toBe('日本語');
  expect(qwenLanguageLabel('auto', 'en')).toBe('Automatic');
  expect(qwenLanguageLabel('NewLanguage', 'en')).toBe('NewLanguage');
});
