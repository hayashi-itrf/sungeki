import { parseArgs } from 'node:util';
import { renderRequest } from './render.ts';
const { values } = parseArgs({ options: { input: { type: 'string' }, 'runtime-root': { type: 'string' } } });
if (!values['runtime-root']) throw new Error('--runtime-root <external runtime folder> が必要です');
if (!values.input) throw new Error('--input <absolute request.json> が必要です');
const controller = new AbortController();
const cancel = () => controller.abort(new Error('VRM描画をキャンセルしました'));
process.once('SIGINT', cancel); process.once('SIGTERM', cancel);
const timeout = setTimeout(() => controller.abort(new Error('VRM描画の制限時間を超えました')), 180000);
try { await renderRequest(values.input, controller.signal, values['runtime-root']); }
catch (error) { console.error(String(error)); process.exitCode = 1; }
finally { clearTimeout(timeout); process.removeListener('SIGINT', cancel); process.removeListener('SIGTERM', cancel); }
