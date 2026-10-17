import { locale } from '../../shared/i18n/locale.svelte';
import type { SpriteContent } from './create-sprite';

// Draft-only operations. Committing still uses SetSprite and host validation.
export function renamePose(value: SpriteContent, index: number, name: string): SpriteContent {
  if (!name.trim() || new TextEncoder().encode(name).length > 256 || /[\u0000-\u001f\u007f-\u009f]/.test(name)) {
    throw new Error(locale.text('spriteNameInvalid'));
  }
  const old = value.sprites.poses[index];
  if (!old) throw new Error(locale.text('spriteMissing'));
  if (value.sprites.poses.some((p, i) => i !== index && p.name === name)) throw new Error(locale.text('spriteDuplicateName'));
  return { ...value, pose: value.pose === old.name ? name : value.pose,
    sprites: { ...value.sprites, default_pose: value.sprites.default_pose === old.name ? name : value.sprites.default_pose,
      poses: value.sprites.poses.map((p, i) => i === index ? { ...p, name } : { ...p }) } };
}
export function addPose(value: SpriteContent): SpriteContent {
  if (value.sprites.poses.length >= 256) throw new Error(locale.text('spriteLimit'));
  const base = value.sprites.poses.find(p => p.name === (value.pose ?? value.sprites.default_pose));
  if (!base) throw new Error(locale.text('spriteDuplicateMissing'));
  let index = 1;
  while (value.sprites.poses.some(p => p.name === `${locale.text('spriteNewPose')} ${index}`)) index++;
  return { ...value, sprites: { ...value.sprites, poses: [
    ...value.sprites.poses.map(p => ({ ...p })), { ...base, name: `${locale.text('spriteNewPose')} ${index}` },
  ] } };
}
export function removePose(value: SpriteContent, index: number): SpriteContent {
  const pose = value.sprites.poses[index];
  if (!pose || value.sprites.poses.length <= 1 || pose.name === value.sprites.default_pose || pose.name === value.pose) {
    throw new Error(locale.text('spriteDeleteError'));
  }
  return { ...value, sprites: { ...value.sprites, poses: value.sprites.poses.filter((_,i) => i !== index).map(p => ({ ...p })) } };
}
