import { readSnapshot } from './snapshot.ts';
import { createHash } from 'node:crypto';
import { inspectKind, parseGlb, type VrmInputKind } from './glb.ts';

export async function readGlb(path: string, kind?: VrmInputKind) {
  const bytes = await readSnapshot(path, 20, 100 * 1024 * 1024);
  const gltf = parseGlb(bytes);
  const inspection = kind ? inspectKind(gltf, kind) : null;
  const meta = gltf.extensions?.VRMC_vrm?.meta ?? gltf.extensions?.VRM?.meta;
  return {
    bytes,
    manifest: {
      path, bytes: bytes.length, sha256: createHash('sha256').update(bytes).digest('hex'), inspection,
      credit: meta ? {
        name: meta.name ?? meta.title, authors: meta.authors ?? [meta.author],
        licenseUrl: meta.licenseUrl ?? meta.otherLicenseUrl,
        otherLicenseUrl: meta.otherLicenseUrl,
        allowRedistribution: meta.allowRedistribution ?? null,
      } : null,
    },
  };
}
