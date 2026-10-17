import type { SubtitleStyle } from '@sungeki/api-client';
import { contrastingTextColor } from '../../shared/text-contrast';

/** Form defaults for a newly registered character; never rewrite saved styles. */
export function newCharacterSubtitles(background: string): SubtitleStyle {
  const color = contrastingTextColor(background);
  return {
    font_size: 64, color,
    layout: { alignment: 'left', wrap: true },
    outline: { width: 3, color: color === '#000000' ? '#ffffff' : '#000000' },
  };
}
