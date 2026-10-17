import { writeFile, unlink } from 'node:fs/promises';
import { spawnSync } from 'node:child_process';

const fixtures = [
  ['__negative_type_probe.ts', 'const value: string = 123;\nexport { value };\n', 'tsc', ['--noEmit']],
  ['__NegativeTypeProbe.svelte', '<script lang="ts">let value: string = 123;</script>\n<p>{value}</p>\n', 'svelte-check', ['--tsconfig', './tsconfig.json']],
];
for (const [name, source, command, args] of fixtures) {
  const path = `apps/desktop/src/${name}`;
  let created = false;
  try {
    await writeFile(path, source, { flag: 'wx' });
    created = true;
    const result = spawnSync(`../../node_modules/.bin/${command}`, args, {
      cwd: 'apps/desktop', encoding: 'utf8',
    });
    const output = `${result.stdout ?? ''}${result.stderr ?? ''}`;
    if (result.error || result.status === 0 || !output.includes(name) || !output.includes('not assignable')) {
      throw new Error(`故意の型エラーを検出できません: ${name}\n${output}`);
    }
    console.log(`型エラー検出成功: ${name}`);
  } finally {
    if (created) await unlink(path);
  }
}
