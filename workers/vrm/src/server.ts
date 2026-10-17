import { createServer } from 'node:http';
import { readFile } from 'node:fs/promises';
import { randomUUID } from 'node:crypto';

export async function serveAssets(model: Buffer, motion: Buffer) {
  const token = randomUUID();
  const runner = await readFile(new URL('../dist/runner.js', import.meta.url));
  const routes = new Map<string, [string, Buffer | string]>([
    [`/${token}/`, ['text/html; charset=utf-8', '<!doctype html><meta charset="utf-8"><script type="module" src="runner.js"></script>']],
    [`/${token}/runner.js`, ['text/javascript', runner]],
    [`/${token}/model`, ['model/gltf-binary', model]],
    [`/${token}/motion`, ['model/gltf-binary', motion]],
  ]);
  const server = createServer((request, response) => {
    const route = routes.get(request.url ?? '');
    if (request.method !== 'GET' || !route) { response.writeHead(404).end(); return; }
    response.writeHead(200, {
      'Content-Type': route[0], 'Cache-Control': 'no-store',
      'Content-Security-Policy': "default-src 'none'; script-src 'self'; connect-src 'self' blob: data:; img-src 'self' blob: data:; style-src 'unsafe-inline'",
    });
    response.end(route[1]);
  });
  await new Promise<void>((resolve, reject) => {
    server.once('error', reject);
    server.listen(0, '127.0.0.1', resolve);
  });
  const address = server.address();
  if (!address || typeof address === 'string') throw new Error('ローカル素材サーバーを起動できません');
  const origin = `http://127.0.0.1:${address.port}`;
  return {
    url: `${origin}/${token}/`,
    allowed: new Set([...routes.keys()].map((path) => origin + path)),
    close: () => new Promise<void>((resolve, reject) => {
      server.closeAllConnections();
      server.close((error) => error ? reject(error) : resolve());
    }),
  };
}
