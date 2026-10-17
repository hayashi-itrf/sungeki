import { parseArgs } from 'node:util';
import { runProbe } from './run.ts';

const { values } = parseArgs({ options: { model: { type: 'string' }, motion: { type: 'string' } } });
if (!values.model || !values.motion) throw new Error('--model <absolute .vrm> --motion <absolute .vrma> が必要です');
const controller = new AbortController();
const cancel = () => controller.abort(new Error('ワーカーをキャンセルしました'));
process.once('SIGINT', cancel);
process.once('SIGTERM', cancel);
const timeout = setTimeout(() => controller.abort(new Error('検証時間の上限を超えました')), 180000);
try {
  await runProbe(values.model, values.motion, controller.signal);
} catch (error) {
  console.error(String(error));
  process.exitCode = 1;
} finally {
  clearTimeout(timeout);
  process.removeListener('SIGINT', cancel);
  process.removeListener('SIGTERM', cancel);
}
