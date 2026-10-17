import { expect, test } from 'vitest';
import { parseDialogue, scriptRows } from './dialogue';
test('readings remain separate from captions and do not count as diagram lines', () => {
  const script = '左： Codexについて。 \r\n読み: コーデックスについて。 \r\n\r\n右:次の話です。';
  expect(parseDialogue(script)).toEqual([
    { role: 'left', subtitle_text: ' Codexについて。 ', spoken_text: ' コーデックスについて。 ', pose: null },
    { role: 'right', subtitle_text: '次の話です。', spoken_text: null, pose: null },
  ]);
  expect(scriptRows(script)).toEqual(['左： Codexについて。 ', '右：次の話です。']);
});
test('orphan, duplicate, blank and separated reading directives fail at their physical line', () => {
  for (const [script, line] of [
    ['読み：こーでっくす', 1], ['左：Codex\n読み： ', 2],
    ['左：Codex\n読み：コーデックス\n読み：別の読み', 3], ['左：Codex\n\n読み：コーデックス', 3],
  ] as const) expect(() => parseDialogue(script)).toThrow(`${line}行目`);
  expect(parseDialogue('左：読み：はセリフ内の文字。')[0]!.subtitle_text).toBe('読み：はセリフ内の文字。');
});

test('sprite pose annotations preserve speech order, exact names and separate pronunciation', () => {
  const lines = parseDialogue('左：Codex\n読み：コーデックス\nポーズ：笑顔\n右：返事\nポーズ: 通常 \n読み：へんじ');
  expect(lines).toHaveLength(2);
  expect(lines[0]).toMatchObject({ sprite_pose: '笑顔', spoken_text: 'コーデックス', subtitle_text: 'Codex' });
  expect(lines[1]).toMatchObject({ sprite_pose: ' 通常 ', spoken_text: 'へんじ' });
  expect(scriptRows('左：本文\nポーズ：笑顔')).toEqual(['左：本文']);
  for (const [script, row] of [['ポーズ：笑顔', 1], ['左：本文\nポーズ：', 2],
    ['左：本文\nポーズ：笑顔\nポーズ：通常', 3], ['左：本文\n\nポーズ：笑顔', 3]] as const) {
    expect(() => parseDialogue(script)).toThrow(`${row}行目`);
  }
});
