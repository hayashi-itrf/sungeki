import { defineConfig } from 'vite';
export default defineConfig({
  build: {
    lib: { entry: 'src/browser.ts', formats: ['es'], fileName: () => 'runner.js' },
    sourcemap: false,
  },
});
