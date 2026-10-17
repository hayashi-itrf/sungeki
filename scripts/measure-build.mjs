import { spawn, execFileSync } from 'node:child_process';
import { mkdir, copyFile, rm, writeFile } from 'node:fs/promises';
import { join, resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { randomUUID } from 'node:crypto';
import { performance } from 'node:perf_hooks';
import { ssdRuntime } from './lib/ssd-runtime.mjs';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const directory = join(await ssdRuntime(), 'build-measurements', randomUUID());
await mkdir(directory, { recursive: true });
const report = { directory, createdAt: new Date().toISOString(),
  rust: execFileSync('rustc', ['-vV'], { encoding: 'utf8', cwd: root }),
  scope: 'workspace release; dependency acquisition excluded; compiler cache empty, OS cache not purged',
  runs: [] };
async function run(command, args, env = process.env) {
  const start = performance.now();
  const child = spawn(command, args, { cwd: root, env, stdio: 'inherit' });
  await new Promise((ok, fail) => {
    child.once('error', fail);
    child.once('exit', (code, signal) => code === 0 ? ok() : fail(new Error(`${command}: ${code ?? signal}`)));
  });
  return (performance.now() - start) / 1000;
}
report.fetchSeconds = await run('cargo', ['fetch', '--locked']);
await run('npm', ['run', 'build']);
await run('npm', ['run', 'prepare:host']); // Tauri sidecar prerequisite, outside timed samples.
for (let index = 1; index <= 3; index++) {
  const target = join(directory, `target-${index}`);
  const env = { ...process.env, CARGO_TARGET_DIR: target, RUSTC_WRAPPER: '', RUSTC_WORKSPACE_WRAPPER: '' };
  const args = ['build', '--workspace', '--release', '--locked', '--offline', '--timings'];
  console.log(`Cold build ${index}/3`);
  const coldSeconds = await run('cargo', args, env);
  await copyFile(join(target, 'cargo-timings/cargo-timing.html'), join(directory, `cold-${index}.html`));
  const targetKiB = Number(execFileSync('du', ['-sk', target], { encoding: 'utf8' }).split(/\s+/)[0]);
  const warmSeconds = await run('cargo', args, env);
  report.runs.push({ index, coldSeconds, warmSeconds, targetKiB });
  await writeFile(join(directory, 'report.json'), JSON.stringify(report, null, 2) + '\n');
  await rm(target, { recursive: true }); // Only this invocation's unique build output.
}
const median = (key) => report.runs.map((run) => run[key]).sort((a, b) => a - b)[1];
report.medianColdSeconds = median('coldSeconds');
report.medianWarmSeconds = median('warmSeconds');
await writeFile(join(directory, 'report.json'), JSON.stringify(report, null, 2) + '\n');
console.log(JSON.stringify(report, null, 2));
