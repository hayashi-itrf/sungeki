import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import type { Plugin } from 'vite';
import { frontendNotices } from './frontend-notices.ts';

// Record actual output modules; build tools and unrelated lock entries are not
// evidence of code shipped in the desktop webview. This is not license clearance.
export function dependencyLocation(id: string): { location: string; module: string } | null {
  const normalized = id.replaceAll('\\', '/').split('?')[0];
  const marker = '/node_modules/';
  const start = normalized.indexOf(marker);
  if (start < 0 || normalized.startsWith('\0')) return null;
  const relative = normalized.slice(start + 1);
  const last = relative.lastIndexOf('node_modules/') + 'node_modules/'.length;
  const parts = relative.slice(last).split('/');
  const count = parts[0].startsWith('@') ? 2 : 1;
  return {
    location: relative.slice(0, last) + parts.slice(0, count).join('/'),
    module: parts.slice(count).join('/'),
  };
}

export function frontendDependencies(root: string): Plugin {
  return {
    name: 'vvm-frontend-dependency-evidence',
    apply: 'build',
    generateBundle(_options, bundle) {
      const lockBytes = readFileSync(resolve(root, 'package-lock.json'));
      const lock = JSON.parse(lockBytes.toString());
      const packages = new Map<string, { name: string; version: string; license: string | null;
        integrity: string | null; modules: Set<string>; chunks: Set<string> }>();
      for (const [chunkName, output] of Object.entries(bundle)) {
        if (output.type !== 'chunk') continue;
        for (const id of Object.keys(output.modules)) {
          const dependency = dependencyLocation(id);
          if (!dependency) continue;
          const { location, module } = dependency;
          const entry = lock.packages[location];
          if (!entry || entry.link) this.error(`Unresolved bundled dependency: ${location}`);
          const manifest = JSON.parse(readFileSync(resolve(root, location, 'package.json'), 'utf8'));
          if (manifest.version !== entry.version) this.error(`Dependency version mismatch: ${location}`);
          const record = packages.get(location) ?? {
            name: manifest.name, version: entry.version, license: entry.license ?? null,
            integrity: entry.integrity ?? null, modules: new Set<string>(), chunks: new Set<string>(),
          };
          record.modules.add(module);
          record.chunks.add(chunkName);
          packages.set(location, record);
        }
      }
      const evidence = {
        scope: 'Desktop JS output module provenance only; not license clearance, native dependencies or external runtime inventory',
        lock_sha256: createHash('sha256').update(lockBytes).digest('hex'),
        packages: [...packages.entries()].sort(([a], [b]) => a.localeCompare(b)).map(([location, item]) => ({
          location, ...item, modules: [...item.modules].sort(), chunks: [...item.chunks].sort(),
        })),
      };
      this.emitFile({ type: 'asset', fileName: 'frontend-dependencies.json',
        source: JSON.stringify(evidence, null, 2) + '\n' });
      this.emitFile({ type: 'asset', fileName: 'THIRD_PARTY_FRONTEND.txt',
        source: frontendNotices(root, evidence.packages) });
    },
  };
}
