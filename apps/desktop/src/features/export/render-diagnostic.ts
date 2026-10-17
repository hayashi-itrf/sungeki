import { locale } from '../../shared/i18n/locale.svelte';

// Translate only the exact, known guidance. Preserve unknown diagnostics verbatim.
const english = new Map<string, string>([
  ['「設定」→「動画制作の実行環境」で外部ディスクの保存フォルダを選んで保存する',
    'In Settings → Video production tools, choose a storage folder on your external disk and save the settings.'],
  ['動画出力の作業領域を利用できません。SSDの接続、保存先フォルダーと書き込み権限を確認して、もう一度開始してください。',
    'The video export workspace is unavailable. Check the SSD connection, destination folder and write permissions, then start again.'],
  ['SSDの接続、作業領域の保存先と書き込み権限を確認する',
    'Check the SSD connection, workspace location and write permissions.'],
  ['修復後に動画出力を開始し直す（起動時の保存先設定を変更した場合はホストの再起動が必要）',
    'Start video export again after fixing the problem. If you changed the startup workspace setting, restart the host first.'],
]);

export function renderDiagnostic(text: string): string {
  return locale.current === 'en' ? english.get(text) ?? text : text;
}
