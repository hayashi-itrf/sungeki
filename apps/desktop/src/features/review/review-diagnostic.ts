import { locale } from '../../shared/i18n/locale.svelte';

const english = new Map<string, string>([
  ['画像検査の作業領域を利用できません。SSDの接続と保存先を確認して、もう一度開始してください。',
    'The image review workspace is unavailable. Check the external disk and storage location, then start again.'],
  ['「設定」→「動画制作の実行環境」で外部ディスクの保存フォルダを選んで保存する',
    'In Settings → Video production tools, choose a storage folder on your external disk and save the settings.'],
  ['SSDの接続と作業領域の設定を確認する',
    'Check the SSD connection and workspace settings.'],
  ['修復後に再度確認を開始する（起動時の環境変数を変更した場合のみホストの再起動が必要）',
    'Start the review again after fixing the problem. Restart the host only if you changed startup environment variables.'],
]);

export function reviewDiagnostic(error: unknown): string {
  const translate = (text: string) => locale.current === 'en' ? english.get(text) ?? text : text;
  if (!error || typeof error !== 'object' || !('message' in error)) return String(error);
  const actions = 'suggested_actions' in error && Array.isArray(error.suggested_actions)
    ? error.suggested_actions.filter((action): action is string => typeof action === 'string') : [];
  return [String(error.message), ...actions].map(translate).join('\n');
}
