import { locale } from '../../shared/i18n/locale.svelte';
import type { ClipContent } from '@sungeki/api-client';
export function clipLabel(content: ClipContent): string {
  switch (content.type) {
    case 'blur': return locale.text('clipBlur');
    case 'video': return locale.text('clipVideo');
    case 'audio': return locale.text('clipAudio');
    case 'text': return content.text;
    case 'speech': return content.subtitle_text || content.spoken_text || locale.text('clipSpeech');
    case 'vrm': return locale.text('clipVrm');
    case 'sprite': return locale.text('clipSprite');
    case 'psd': return locale.text('clipPsd');
    case 'image': return locale.text('clipImage');
    case 'shape': return locale.text('clipShape');
  }
}
