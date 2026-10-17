import { randomUUID } from 'node:crypto';
import { readFileSync } from 'node:fs';
import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';
import { fileURLToPath } from 'node:url';
import { frontendDependencies } from '../../scripts/frontend-dependencies';

const buildInfo = {
  version: JSON.parse(readFileSync(new URL('./package.json', import.meta.url), 'utf8')).version,
  builtAt: new Date().toISOString(),
  id: randomUUID(),
};

export default defineConfig({
  define: { __APP_BUILD_INFO__: JSON.stringify(buildInfo) },
  plugins: [{ name: 'app-build-info', generateBundle() {
    this.emitFile({ type: 'asset', fileName: 'build-info.json', source: JSON.stringify(buildInfo, null, 2) });
  } }, svelte(), frontendDependencies(fileURLToPath(new URL('../../', import.meta.url)))],
  server: { port: 1424, strictPort: true },
  preview: { port: 2424, strictPort: true },
  clearScreen: false,
});
