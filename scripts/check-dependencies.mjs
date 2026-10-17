import { execFileSync } from 'node:child_process';

const allowed = {
  project: [], editor: ['project'], scene: ['project'],
  protocol: ['project', 'editor'], 'host-client': ['protocol'],
  storage: ['project'], assets: ['project'], media: ['project'],
  speech: ['project', 'assets', 'media'],
  render: ['project', 'scene', 'assets', 'media'],
  review: ['project', 'scene', 'assets', 'render', 'media'],
  jobs: ['project'],
  host: ['project', 'editor', 'protocol', 'storage', 'assets', 'scene', 'media', 'speech', 'render', 'review', 'jobs'],
  mcp: ['protocol', 'host-client'], desktop: ['protocol', 'host-client'],
};
const metadata = JSON.parse(execFileSync('cargo', ['metadata', '--no-deps', '--format-version', '1', '--locked'], { encoding: 'utf8' }));
let failed = false;
for (const pkg of metadata.packages) {
  const role = pkg.name.replace(/^sungeki-/, '');
  if (!Object.hasOwn(allowed, role)) throw new Error(`未登録の責務: ${pkg.name}`);
  for (const dep of pkg.dependencies.filter((d) => d.name.startsWith('sungeki-'))) {
    if (!allowed[role].includes(dep.name.replace(/^sungeki-/, ''))) {
      console.error(`依存方向違反: ${pkg.name} → ${dep.name}`);
      failed = true;
    }
  }
}
console.log(`依存方向: ${failed ? '不合格' : '合格'}`);
if (failed) process.exitCode = 1;
