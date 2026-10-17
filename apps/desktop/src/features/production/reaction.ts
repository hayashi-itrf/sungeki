import { locale } from '../../shared/i18n/locale.svelte';
import type { BuildReaction } from '@sungeki/api-client';
export type ReactionInput = {
  x: number | undefined; y: number | undefined; width: number | undefined; height: number | undefined;
  scale: number | undefined; fadeIn: number | undefined; fadeOut: number | undefined;
};
export function newReaction(portrait: boolean): ReactionInput {
  return { x: portrait ? 540 : 960, y: 300, width: 240, height: 240, scale: 50, fadeIn: 0.15, fadeOut: 0.15 };
}
export function reactionFromForm(input: ReactionInput): BuildReaction {
  const { x, y, width, height, scale, fadeIn, fadeOut } = input;
  if (![x, y, width, height, scale, fadeIn, fadeOut].every(n => typeof n === 'number' && Number.isFinite(n))
      || width! <= 0 || height! <= 0 || scale! <= 0 || scale! > 100 || fadeIn! < 0 || fadeOut! < 0) {
    throw new Error(locale.text('productionValidationReactionSettings'));
  }
  const times = [fadeIn!, fadeOut!].map(seconds => Math.round(seconds * 1_000_000));
  if (times.some(t => !Number.isSafeInteger(t)) || times[0]! + times[1]! > 604800000000) {
    throw new Error(locale.text('productionValidationReactionFade'));
  }
  return { center_x: x!, center_y: y!, width: width!, height: height!, start_scale: scale! / 100,
    fade: { in_us: times[0]!, out_us: times[1]! } };
}
