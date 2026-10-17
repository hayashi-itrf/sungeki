import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

interface Dependency { name: string; version: string; integrity: string | null }
interface Notice extends Dependency { file: string; sha256: string; selected_license: string }

export function frontendNotices(root: string, dependencies: Dependency[]): string {
  const directory = resolve(root, 'licenses/frontend');
  const manifest = JSON.parse(readFileSync(resolve(directory, 'manifest.json'), 'utf8'));
  const sections = ['THIRD-PARTY NOTICES — DESKTOP FRONTEND\n'
    + 'Native libraries and external runtimes have separate notices.\n'];
  for (const dependency of dependencies) {
    const matches: Notice[] = manifest.packages.filter((item: Notice) => item.name === dependency.name
      && item.version === dependency.version && item.integrity === dependency.integrity);
    if (matches.length !== 1) throw new Error(`Missing or ambiguous frontend notice: ${dependency.name} ${dependency.version}`);
    const notice = matches[0];
    if (!/^[\w.-]+$/.test(notice.file)) throw new Error('Invalid frontend notice filename');
    const bytes = readFileSync(resolve(directory, notice.file));
    if (createHash('sha256').update(bytes).digest('hex') !== notice.sha256) {
      throw new Error(`Frontend notice checksum mismatch: ${dependency.name}`);
    }
    sections.push(`\n${dependency.name} ${dependency.version} (${notice.selected_license})\n\n${bytes.toString('utf8')}\n`);
  }
  return sections.join('');
}
