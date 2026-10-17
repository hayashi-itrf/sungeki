import { locale } from '../../shared/i18n/locale.svelte';
import { describeStructure } from './structure-description';
import { describeMedia } from './media-description';
import { describeCharacter } from './character-description';
import { clipLabel } from '../editor/clip-label';
import type { Command, Project } from '@sungeki/api-client';
export function describeCommand(command: Command, project: Project): string {
  if (command.type === 'ripple') return locale.text('rippleDescription')
    .replace('{count}', String(command.clip_ids.length))
    .replace('{boundary}', String(command.boundary_us / 1e6))
    .replace('{distance}', String(Math.abs(command.delta_us) / 1e6))
    .replace('{direction}', locale.text(command.delta_us < 0 ? 'structureEarlier' : 'structureBackward'));
  if (locale.current === 'en') {
    const description = describeStructure(command, project) ?? describeMedia(command, project);
    if (description !== null) return description;
  }
  const scene = (id: string) => {
    const value = project.scenes.find(s => s.id === id);
    return value ? `「${value.name}」（${value.start_us / 1_000_000}秒）` : '指定シーン';
  };
  const group = (id: string) => `「${project.groups.find(g => g.id === id)?.name ?? '指定グループ'}」`;
  switch (command.type) {
    case 'set_text_outline': return command.outline ? `文字の縁取りを${command.outline.width}px・${command.outline.color}に設定` : '文字の縁取りを解除する';
    case 'set_text_layout': return command.layout ? `文字を${({ left: '左寄せ', center: '中央寄せ', right: '右寄せ' })[command.layout.alignment]}・自動折り返し${command.layout.wrap ? 'あり' : 'なし'}に設定` : '文字組みの指定を解除する';
    case 'set_video': return `動画の使用区間を${command.playback.source_start_us / 1e6}秒から${command.playback.source_duration_us / 1e6}秒間に設定（${command.playback.looped ? '繰り返す' : '繰り返さない'}・${command.playback.audio_stream === null ? '元音声なし' : '選択した元音声を使用'}）`;
    case 'set_transform': {
      const t = command.transform; const m = t.motion;
      return `配置を X ${t.x}・Y ${t.y}・幅 ${t.width}・高さ ${t.height} に設定。${m ? `移動先 X ${m.end_x}・Y ${m.end_y}、終了倍率 ${m.end_scale}、${m.easing === 'linear' ? '等速' : '滑らかな動き'}` : '移動・拡大なし'}`;
    }
    case 'set_scene_fade': return command.fade ? locale.text('sceneFadeOperation')
      .replace('{in}', String(command.fade.in_us / 1e6)).replace('{out}', String(command.fade.out_us / 1e6)) : locale.text('sceneFadeRemove');
    case 'prepend_foreword': return locale.text('forewordOperation').replace('{seconds}', String(command.template.duration_us / 1e6));
    case 'refresh_scene_credits': return `シーン${scene(command.scene_id)}の素材・話者のクレジットで、選択した文字の文章を置き換える`;
    case 'fit_visual_to_speech': return '選択した画像・立ち絵・文字・図形の表示区間を、最初のセリフの開始から最後のセリフの終了に合わせる';
    case 'apply_dialogue_template': return `掛け合い「${command.template.title}」を${command.template.start_us/1e6}秒から作成。発話${command.template.speech_clip_ids.length}件、背景・立ち絵2枚・BGMを配置し、タイトルとクレジットを追加`;
    case 'set_character': return describeCharacter(command.character, project);
    case 'layout_speech': return `セリフ${command.clip_ids.length}件を生成済み音声の長さで、${command.start_us / 1_000_000}秒から間隔${command.gap_us / 1_000_000}秒で配置`;
    case 'move_selection': return `シーン${command.scene_ids.map(scene).join('・') || 'なし'}、グループ${command.group_ids.map(group).join('・') || 'なし'}、個別クリップ${command.clip_ids.length}件を${Math.abs(command.delta_us) / 1_000_000}秒${command.delta_us < 0 ? '前' : '後ろ'}へ移動（重複は一度だけ）`;
    case 'add_scene': return `シーン「${command.scene.name}」を${command.scene.start_us / 1_000_000}秒から${command.scene.duration_us / 1_000_000}秒間で作成`;
    case 'rename_scene': return `シーン${scene(command.scene_id)}の名前を「${command.name}」へ変更`;
    case 'move_scene': return `シーン${scene(command.scene_id)}と所属クリップを${command.start_us / 1_000_000}秒へ移動`;
    case 'resize_scene': return `シーン${scene(command.scene_id)}の長さを${command.duration_us / 1_000_000}秒へ変更`;
    case 'delete_scene': return `シーン${scene(command.scene_id)}と所属クリップ・グループを削除`;
    case 'ungroup_scene': return `シーン${scene(command.scene_id)}だけ解除し、クリップの時刻を維持`;
    case 'duplicate_scene': return `シーン${scene(command.scene_id)}を${command.start_us / 1_000_000}秒へ複製（クリップ${Object.keys(command.clip_ids).length}件・グループ${Object.keys(command.group_ids).length}件）`;
    case 'layout_scenes': return `${command.scene_ids.map(scene).join(' → ')}の順で、${command.start_us / 1_000_000}秒から間隔${command.gap_us / 1_000_000}秒で配置`;
    case 'add_group': return `グループ「${command.group.name}」をクリップ${command.group.clip_ids.length}件で作成`;
    case 'rename_group': return `グループ${group(command.group_id)}の名前を「${command.name}」へ変更`;
    case 'move_group': return `グループ${group(command.group_id)}を${command.start_us / 1_000_000}秒へ移動`;
    case 'duplicate_group': return `グループ${group(command.group_id)}を${command.start_us / 1_000_000}秒へ複製（クリップ${Object.keys(command.clip_ids).length}件）`;
    case 'ungroup': return `グループ${group(command.group_id)}だけ解除`;
    case 'set_clip_scene': {
      const clip = project.clips.find(c => c.id === command.clip_id);
      const label = clip ? clipLabel(clip.content) : '指定クリップ';
      return `クリップ「${label}」の所属を${command.scene_id ? scene(command.scene_id) : 'シーンなし'}へ変更`;
    }
    case 'set_reaction_motion': return `画像を中心（${command.center_x}, ${command.center_y}）で拡大し、${command.width} × ${command.height}にするリアクション演出（開始倍率${command.start_scale}、出現${command.fade.in_us / 1e6}秒・消失${command.fade.out_us / 1e6}秒）。既存の移動・拡大とフェードを置き換え` ;
    default: return locale.current === 'en' ? 'Apply the shared editing command.' : '共通編集コマンドを適用';
  }
}
