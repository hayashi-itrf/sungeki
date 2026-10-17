import { spawn } from 'node:child_process';
import { readFile, access } from 'node:fs/promises';
import { parseArgs } from 'node:util';
import assert from 'node:assert/strict';
import { join } from 'node:path';

const { values } = parseArgs({ options: { model: { type: 'string' }, motion: { type: 'string' } } });
if (!values.model || !values.motion) throw new Error('--model /abs.vrm --motion /abs.vrma が必要です');

async function check(name, motion, cancel) {
  const child = spawn(process.execPath, ['--experimental-strip-types', 'workers/vrm/src/main.ts',
    '--model', values.model, '--motion', motion], { stdio: ['ignore', 'pipe', 'pipe'] });
  let output = '';
  let error = '';
  let cancelled = false;
  child.stdout.on('data', (chunk) => {
    output += chunk;
    if (cancel && !cancelled && output.includes('frame_rendered')) {
      cancelled = true;
      child.kill('SIGTERM');
    }
  });
  child.stderr.on('data', (chunk) => { error += chunk; });
  const timeout = setTimeout(() => child.kill('SIGTERM'), 60000);
  const code = await new Promise((resolve, reject) => {
    child.once('error', reject);
    child.once('exit', resolve);
  }).finally(() => clearTimeout(timeout));
  assert.equal(code, 1, error);
  const result = output.trim().split('\n').map((line) => JSON.parse(line)).find((item) => item.type === 'result');
  assert.ok(result, output + error);
  const report = JSON.parse(await readFile(join(result.directory, 'report.json'), 'utf8'));
  assert.equal(report.status, cancel ? 'cancelled' : 'failed');
  assert.equal(report.cleanupPassed, true);
  assert.deepEqual(report.remainingProcesses, []);
  assert.ok(report.processes.length > 0, 'real browser must have started');
  if (!cancel) assert.match(report.error, /単一のVRMA/);
  await assert.rejects(access(join(result.directory, 'browser-profile')));
  await assert.rejects(access(join(result.directory, 'tmp')));
  console.log(JSON.stringify({ name, status: 'passed', directory: result.directory }));
}
await check('invalid-motion-after-browser-start', values.model, false);
await check('SIGTERM-during-rendering', values.motion, true);
