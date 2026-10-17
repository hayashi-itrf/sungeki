import type { ClipContent, Voice } from '@sungeki/api-client';
import zundamon from '../../../../../presets/characters/zundamon-sakamoto-2.3-mouth.json';
import art from '../../../../../presets/characters/zundamon-sakamoto-2.3-art-mouth.json';
import metan from '../../../../../presets/characters/metan-sakamoto-2.1-mouth.json';

// Use the same bundled metadata as the host to choose the request's ID.
// This is not acceptance validation: the host checks the original, pose,
// mouth binding, voice and credits again before preparing the project.
export function standingPreset(content: ClipContent | undefined, engine: Voice['type'] = 'voicevox') {
  if (content?.type !== 'psd') return undefined;
  return [engine === 'voicevox' ? zundamon : art, metan].find(preset => preset.source_sha256 === content.pose.source_sha256);
}
