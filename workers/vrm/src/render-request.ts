import { validatePose } from '../../../packages/vrm/src/pose.ts';
import { isAbsolute } from 'node:path';
import { readSnapshot } from './snapshot.ts';
import { vrmClock } from '../../../packages/vrm/src/clock.ts';
import type { JointPose, ExpressionWeights } from '@sungeki/vrm';
export type RenderSource = { path: string; sha256: string; byte_length: number };
export type RenderRequest = {
  version: 1; model: RenderSource; motion: RenderSource; width: number; height: number;
  frames: { time_us: number; expressions: ExpressionWeights; pose?: JointPose }[];
};
function object(value: unknown, keys: string[]): Record<string, unknown> {
  if (!value || typeof value !== 'object' || Array.isArray(value)
    || Object.keys(value).some(k => !keys.includes(k)) || keys.some(k => !Object.hasOwn(value, k))) {
    throw new Error('VRM描画要求の項目が不正です');
  }
  return value as Record<string, unknown>;
}
function source(value: unknown): RenderSource {
  const s = object(value, ['path','sha256','byte_length']);
  if (typeof s.path !== 'string' || !isAbsolute(s.path) || s.path.includes('\0')
    || typeof s.sha256 !== 'string' || !/^[a-f0-9]{64}$/.test(s.sha256)
    || !Number.isSafeInteger(s.byte_length) || (s.byte_length as number) < 20 || (s.byte_length as number) > 100*1024*1024) {
    throw new Error('VRM原本の参照情報が不正です');
  }
  return s as RenderSource;
}
export function parseRenderRequest(value: unknown): RenderRequest {
  const r = object(value, ['version','model','motion','width','height','frames']);
  if (r.version !== 1 || ![r.width,r.height].every(v => Number.isInteger(v) && (v as number) >= 1 && (v as number) <= 2048)
    || !Array.isArray(r.frames) || r.frames.length < 1 || r.frames.length > 100000) throw new Error('VRM描画要求の版・寸法・件数が不正です');
  const frames = r.frames.map(value => {
    const f = object({ pose: {}, ...(value as object) }, ['time_us','expressions','pose']);
    validatePose(f.pose as JointPose);
    vrmClock(f.time_us as number);
    if (!f.expressions || typeof f.expressions !== 'object' || Array.isArray(f.expressions)) throw new Error('表情指定が不正です');
    const entries = Object.entries(f.expressions);
    if (entries.length > 256 || entries.some(([k,v]) => !k || k.length > 256 || /[\u0000-\u001f\u007f]/.test(k)
      || typeof v !== 'number' || !Number.isFinite(v) || v < 0 || v > 1)) throw new Error('表情指定が不正です');
    return { pose: structuredClone(f.pose) as JointPose, time_us: f.time_us as number, expressions: Object.fromEntries(entries) as ExpressionWeights };
  });
  return { version: 1, model: source(r.model), motion: source(r.motion), width: r.width as number, height: r.height as number, frames };
}
export async function readRenderRequest(path: string) {
  return parseRenderRequest(JSON.parse(new TextDecoder('utf-8', { fatal: true }).decode(await readSnapshot(path, 2, 32*1024*1024))));
}
