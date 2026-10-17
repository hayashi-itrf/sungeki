import assert from 'node:assert/strict';
import { mkdtempSync, mkdirSync, writeFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { test } from 'node:test';
import { createHash } from 'node:crypto';
import { dependencyLocation, frontendDependencies } from './frontend-dependencies.ts';
import { frontendNotices } from './frontend-notices.ts';

test('scoped, nested and virtual module identities', () => {
  assert.deepEqual(dependencyLocation('/repo/node_modules/@scope/a/src.js?x'),
    { location: 'node_modules/@scope/a', module: 'src.js' });
  assert.deepEqual(dependencyLocation('/repo/node_modules/a/node_modules/b/index.js'),
    { location: 'node_modules/a/node_modules/b', module: 'index.js' });
  assert.equal(dependencyLocation('/repo/src/main.ts'), null);
  assert.equal(dependencyLocation('\0/repo/node_modules/a/index.js'), null);
});

test('emits only output dependencies without host paths; rejects unaccounted versions', () => {
  const root = mkdtempSync(join(tmpdir(), 'vvm-provenance-'));
  try {
    mkdirSync(join(root, 'node_modules/a'), { recursive: true });
    mkdirSync(join(root, 'licenses/frontend'), { recursive: true });
    const notice = 'fixture copyright and permission';
    writeFileSync(join(root, 'licenses/frontend/a.txt'), notice);
    writeFileSync(join(root, 'licenses/frontend/manifest.json'), JSON.stringify({ packages: [{
      name: 'a', version: '1', integrity: null, selected_license: 'MIT', file: 'a.txt',
      sha256: createHash('sha256').update(notice).digest('hex'),
    }] }));
    writeFileSync(join(root, 'node_modules/a/package.json'), JSON.stringify({ name: 'a', version: '1' }));
    writeFileSync(join(root, 'package-lock.json'), JSON.stringify({ packages: {
      'node_modules/a': { version: '1', license: 'MIT' },
      'node_modules/build-only': { version: '2', license: 'MIT' },
    } }));
    const plugin = frontendDependencies(root);
    let emitted;
    const context = { emitFile: x => { if (x.fileName === 'frontend-dependencies.json') emitted = x; },
      error: x => { throw new Error(x); } };
    const bundle = { 'app.js': { type: 'chunk', modules: {
      [join(root, 'node_modules/a/index.js')]: {}, [join(root, 'src/main.ts')]: {},
    } } };
    plugin.generateBundle.call(context, {}, bundle);
    const evidence = JSON.parse(emitted.source);
    assert.deepEqual(evidence.packages.map(x => x.name), ['a']);
    assert.equal(emitted.source.includes(root), false);
    assert.match(frontendNotices(root, evidence.packages), /fixture copyright and permission/);
    assert.throws(() => frontendNotices(root, [{ name: 'a', version: '9', integrity: null }]), /Missing/);
    writeFileSync(join(root, 'licenses/frontend/a.txt'), 'changed');
    assert.throws(() => frontendNotices(root, evidence.packages), /checksum mismatch/);
    writeFileSync(join(root, 'node_modules/a/package.json'), JSON.stringify({ name: 'a', version: '2' }));
    assert.throws(() => plugin.generateBundle.call(context, {}, bundle), /version mismatch/);
    bundle['app.js'].modules = { [join(root, 'node_modules/unknown/index.js')]: {} };
    assert.throws(() => plugin.generateBundle.call(context, {}, bundle), /Unresolved bundled dependency/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});
