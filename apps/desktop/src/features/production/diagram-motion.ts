import { locale } from '../../shared/i18n/locale.svelte';
import type { MotionEasing, VisualMotion } from '@sungeki/api-client';
export type DiagramMotionInput = { endX: number | undefined; endY: number | undefined; scale: number | undefined; easing: MotionEasing };
export function newDiagramMotion(): DiagramMotionInput {
  return { endX: undefined, endY: undefined, scale: 100, easing: 'ease_in_out' };
}
export function diagramMotionFromForm(input: DiagramMotionInput): VisualMotion {
  const { endX, endY, scale, easing } = input;
  if (endX === undefined || endY === undefined || scale === undefined || ![endX, endY, scale].every(Number.isFinite)
      || Math.abs(endX) > 1e6 || Math.abs(endY) > 1e6 || scale <= 0 || !['linear', 'ease_in_out'].includes(easing)) {
    throw new Error(locale.text('productionValidationDiagramMotion'));
  }
  return { end_x: endX, end_y: endY, end_scale: scale / 100, easing };
}
