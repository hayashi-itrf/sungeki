import { locale } from '../../shared/i18n/locale.svelte';

const storagePrefix = '保存フォルダを確認できません。外部ディスク内の既存フォルダを選び直してください: ';

// Translate only the known host explanation, preserving paths and OS details.
export function runtimeDiagnostic(message: string): string {
  if (locale.current !== 'en' || !message.startsWith(storagePrefix)) return message;
  return 'Cannot access the storage folder. Choose an existing folder on your external disk: '
    + message.slice(storagePrefix.length);
}
