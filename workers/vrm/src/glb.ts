/** Container validation. Semantic rig/animation validation still belongs to the shared VRM loader. */
export function parseGlb(bytes: Buffer) {
  if (bytes.length < 20 || bytes.readUInt32LE(0) !== 0x46546c67
    || bytes.readUInt32LE(4) !== 2 || bytes.readUInt32LE(8) !== bytes.length) {
    throw new Error('GLB 2.0ではありません');
  }
  let offset = 12;
  let json: Buffer | undefined;
  let binary: Buffer | undefined;
  while (offset < bytes.length) {
    if (offset + 8 > bytes.length) throw new Error('GLB chunkヘッダーが不足しています');
    const length = bytes.readUInt32LE(offset), kind = bytes.readUInt32LE(offset + 4);
    if (length % 4 !== 0 || length > bytes.length - offset - 8) throw new Error('GLB chunk長が不正です');
    const data = bytes.subarray(offset + 8, offset + 8 + length);
    if (offset === 12 && kind !== 0x4e4f534a) throw new Error('GLB先頭chunkはJSONである必要があります');
    if (kind === 0x4e4f534a) {
      if (json) throw new Error('GLB JSON chunkが重複しています');
      json = data;
    } else if (kind === 0x004e4942) {
      if (binary) throw new Error('GLB BIN chunkが重複しています');
      binary = data;
    }
    offset += 8 + length;
  }
  const gltf = JSON.parse(new TextDecoder('utf-8', { fatal: true }).decode(json));
  if (!gltf || gltf.asset?.version !== '2.0') throw new Error('glTF asset.versionは2.0が必要です');
  for (const key of ['buffers', 'images']) {
    const entries = gltf[key] ?? [];
    if (!Array.isArray(entries)) throw new Error(`glTF ${key}は配列が必要です`);
    for (const [index, entry] of entries.entries()) {
      if (!entry || typeof entry !== 'object') throw new Error(`glTF ${key}の要素が不正です`);
      if (entry.uri !== undefined && (typeof entry.uri !== 'string' || !entry.uri.startsWith('data:'))) {
        throw new Error('外部参照なしのGLBだけを扱います');
      }
      if (key === 'buffers') {
        if (!Number.isSafeInteger(entry.byteLength) || entry.byteLength < 1) throw new Error('GLB buffer長が不正です');
        if (entry.uri === undefined && (index !== 0 || !binary || entry.byteLength > binary.length
          || binary.length - entry.byteLength > 3)) throw new Error('GLB BINとbuffer長が一致しません');
      }
    }
  }
  return gltf;
}

export type VrmInputKind = 'model' | 'motion';

export function inspectKind(gltf: ReturnType<typeof parseGlb>, kind: VrmInputKind) {
  const extensions = gltf.extensions ?? {};
  if (kind === 'motion') {
    const motion = extensions.VRMC_vrm_animation;
    if (!motion || motion.specVersion !== '1.0' || !motion.humanoid
      || !Array.isArray(gltf.animations) || gltf.animations.length !== 1) {
      throw new Error('単一animationを持つVRMA 1.0が必要です');
    }
    return { kind, version: '1.0', expressions: [] as string[] };
  }
  const modern = extensions.VRMC_vrm, legacy = extensions.VRM;
  if (Boolean(modern) === Boolean(legacy) || !(modern ?? legacy)?.humanoid
    || (modern && modern.specVersion !== '1.0') || (legacy && legacy.specVersion !== '0.0')) {
    throw new Error('VRM 0.0または1.0のモデルが必要です');
  }
  // Expose names only for VRM 1.0; VRM 0 presets are normalized by the shared loader.
  const expressions = modern ? Object.keys({ ...modern.expressions?.preset, ...modern.expressions?.custom }).sort() : [];
  return { kind, version: modern ? '1.0' : '0.0', expressions };
}
