import { locale } from '../../shared/i18n/locale.svelte';
import { englishCollections, englishFields, englishActions } from './difference-english';
import { voiceSettingLabels } from '../scenes/character-description';
import { clipLabel } from './clip-label';
import type { Project, ProjectDifference, ValuePreview } from '@sungeki/api-client';
export function differenceSubject(entry: ProjectDifference, project: Project): string {
  if (!entry.entity_id) return '';
  if (entry.collection === 'clips') {
    const index = project.clips.findIndex(c => c.id === entry.entity_id);
    const clip = project.clips[index];
    if (clip) return `${locale.current === 'en' ? 'Clip' : 'クリップ'} ${index + 1}（${clipLabel(clip.content).slice(0, 40)}）`;
  }
  const named = [...project.tracks, ...project.scenes, ...project.groups, ...project.characters].find(c => c.id === entry.entity_id);
  return named?.name ?? `${locale.current === 'en' ? 'Item absent from this project' : '現在の文書にない項目'}（${entry.entity_id.slice(0, 8)}）`;
}
const collections: Record<string, string> = { project: '作品', tracks: 'トラック', clips: 'クリップ', scenes: 'シーン', groups: 'グループ', assets: '素材', characters: 'キャラクター' };
const fields: Record<string, string> = { outline: '縁取り', playback: '再生設定', source_start_us: '原本の使用開始', source_duration_us: '原本の使用長さ', looped: '繰り返し', audio_stream: '動画内の音声', gain_db: '音量', title: '作品名', name: '名前', content: '内容', text: '文章', transform: '配置', x: '横位置', y: '縦位置', width: '幅', height: '高さ', opacity: '不透明度', flip_x: '左右反転', fade: '映像フェード', motion: '移動・拡大', end_x: '移動先X', end_y: '移動先Y', end_scale: '終了倍率', easing: '動き方', in_us: 'フェードイン', out_us: 'フェードアウト', start_us: '開始時刻', duration_us: '長さ', color: '文字色', font_size: '文字サイズ', fill: '塗り色', type: '種類', shape: '形状', canvas: '画面', fps: 'フレームレート', numerator: '分子', denominator: '分母', background: '背景色', visible: '表示', muted: 'ミュート', kind: '種類', order: '並び順', scene_id: '所属シーン', track_id: '所属トラック', clip_ids: '所属クリップ', audio_mix: '音量設定', pronunciation_dictionary: '読み方辞書', normalization: '音量自動調整' };
export function differenceLabel(entry: ProjectDifference): string {
  if (locale.current === 'en') {
    const field = entry.field.split('/').filter(Boolean).map((f, index) =>
      f === 'voice' && index > 0 ? 'Speaker' : englishFields[f] ?? 'Settings').join(' / ');
    return `${englishCollections[entry.collection] ?? 'Item'}${field ? ' / ' + field : ''}: ${englishActions[entry.kind]}`;
  }
  const action = { added: '追加', removed: '削除', changed: '変更', reordered: '並び替え' }[entry.kind];
  const field = entry.field.split('/').filter(Boolean).map((f, index) => index === 0 && f === 'voice' ? '音声' : fields[f] ?? voiceSettingLabels[f] ?? ({ voice: '音声', settings: '設定', audio: '生成音声', identity: '原本識別情報' } as Record<string, string>)[f] ?? '設定').join('・');
  return `${collections[entry.collection] ?? '項目'}${field ? `の${field}` : ''}：${action}`;
}
export function differenceValue(value: ValuePreview | null, entry: ProjectDifference): string {
  const text = (ja: string, en: string) => locale.current === 'en' ? en : ja;
  if (!value) return text('なし', 'None');
  if (entry.kind === 'added' || entry.kind === 'removed') return text('項目あり', 'Item present');
  if (entry.kind === 'reordered') return text('並び順が異なります', 'Order differs');
  if (value.truncated) return text('長い値のため省略（作品内で確認してください）', 'Value too long to display; inspect it in the project');
  try {
    const parsed: unknown = JSON.parse(value.json);
    if (parsed === null) return text('未設定', 'Not set');
    if (typeof parsed === 'boolean') return parsed ? text('有効', 'Enabled') : text('無効', 'Disabled');
    if (typeof parsed === 'number' && /_(us)$/.test(entry.field)) return `${parsed / 1_000_000}${text(' 秒', ' s')}`;
    if (typeof parsed === 'string' && /_id$/.test(entry.field)) return text('参照先が変更されています', 'Reference changed');
    if (Array.isArray(parsed)) return `${parsed.length}${text(' 件', ' items')}`;
    if (typeof parsed === 'object' && entry.field.split('/').at(-1) === 'fade') {
      const fade = parsed as Record<string, unknown>;
      const duration = (n: unknown): n is number => typeof n === 'number' && Number.isSafeInteger(n) && n >= 0;
      if (Object.keys(fade).length === 2 && duration(fade.in_us) && duration(fade.out_us)) {
        return text(`フェードイン ${fade.in_us / 1e6} 秒、フェードアウト ${fade.out_us / 1e6} 秒`,
          `Fade in ${fade.in_us / 1e6} s, fade out ${fade.out_us / 1e6} s`);
      }
    }
    if (typeof parsed === 'object') return text('設定内容', 'Settings');
    return String(parsed);
  } catch { return text('表示できない値', 'Cannot display value'); }
}
