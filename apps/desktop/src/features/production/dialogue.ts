import { locale } from '../../shared/i18n/locale.svelte';
import type { BuildLine } from '@sungeki/api-client';
// This is a form notation, translated to the common BuildLine DTO only.
// Synthesis, pronunciation dictionaries and timing remain host responsibilities.
export function parseDialogue(script: string): BuildLine[] {
  const lines: BuildLine[] = [];
  let previous: BuildLine | undefined;
  // Rows are separated by CR/LF. Preserve Unicode separators inside host-accepted text.
  for (const [index, row] of script.replace(/\r\n?/g, '\n').split('\n').entries()) {
    if (!row.trim()) { previous = undefined; continue; }
    const spritePose = /^(?:ポーズ|pose)[：:]([^\r\n]*)$/.exec(row);
    if (spritePose) {
      if (!previous || previous.sprite_pose !== undefined || !spritePose[1]!.trim()) throw new Error(locale.text('productionValidationPoseError').replace('{line}', String(index + 1)));
      previous.sprite_pose = spritePose[1]!;
      continue;
    }
    const reading = /^(?:読み|reading)[：:]([^\r\n]*)$/.exec(row);
    if (reading) {
      if (!previous || previous.spoken_text !== null || !reading[1]!.trim()) throw new Error(locale.text('productionValidationReadingError').replace('{line}', String(index + 1)));
      previous.spoken_text = reading[1]!;
      continue;
    }
    const match = /^(左|右|left|right)[：:]([^\r\n]*)$/.exec(row);
    if (!match || !match[2]!.trim()) throw new Error(locale.text('productionValidationDialogueError').replace('{line}', String(index + 1)));
    previous = { role: ['左', 'left'].includes(match[1]!) ? 'left' : 'right', subtitle_text: match[2]!, spoken_text: null, pose: null };
    lines.push(previous);
  }
  return lines;
}
export function scriptRows(script: string): string[] {
  return parseDialogue(script).map(line => `${locale.current === 'en' ? line.role : line.role === 'left' ? '左' : '右'}：${line.subtitle_text}`);
}
