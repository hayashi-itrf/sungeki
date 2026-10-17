import { locale } from '../../shared/i18n/locale.svelte';
import type { MessageKey } from '../../shared/i18n/messages';

const stages: Record<string, MessageKey> = {
  '軽量動画の生成待ち': 'proxyWaiting',
  '軽量動画を生成・検証中': 'proxyWorking',
  '書き出しなどの処理を待っています': 'proxyForeground',
  '軽量動画の検証完了': 'proxyVerifiedStage',
  '軽量動画を生成中': 'proxyGeneratingStage',
};

// Preserve unfamiliar host diagnostics instead of hiding potentially useful text.
export const proxyStage = (stage: string) => stages[stage] ? locale.text(stages[stage]) : stage;
