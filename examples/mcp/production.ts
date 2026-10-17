import { readFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import type { StartBuildVideoArgs, BuildVideoJob, SessionResponse } from '../../packages/api-client/src/generated.ts';
import { McpClient } from './client.ts';

const [command, value] = process.argv.slice(2);
if (!['start', 'watch', 'status', 'cancel', 'capabilities'].includes(command ?? '')
  || (command !== 'capabilities' && !value)) {
  console.error('Usage: production.ts capabilities | start REQUEST.json | watch JOB_ID | status JOB_ID | cancel JOB_ID');
  process.exit(2);
}
const binary = process.env.SUNGEKI_MCP_BINARY
  ?? fileURLToPath(new URL('../../target/release/sungeki-mcp', import.meta.url));
const client = new McpClient(binary);
function jobFrom(value: unknown): BuildVideoJob {
  const response = value as SessionResponse;
  if (response.type !== 'build_video_job') throw new Error('Unexpected build response');
  return response.job;
}
function report(job: BuildVideoJob) {
  console.log(JSON.stringify({
    job_id: job.job_id, session_id: job.session_id, phase: job.phase,
    completed_steps: job.completed_steps, total_steps: job.total_steps,
    progress: job.progress, stale: job.stale, error: job.error,
    output: job.output && {
      project_path: job.output.project_path, video_path: job.output.render.path,
      review_job_id: job.output.review_job_id, review: job.output.review.diagnostics.summary,
    },
  }));
}
try {
  await client.initialize();
  if (command === 'capabilities') {
    console.log(JSON.stringify(await client.call('get_capabilities', {})));
  } else if (command === 'start') {
    // The JSON is the public StartBuildVideoArgs, not an old-format project.
    // Host schema validation is authoritative. Keep this exact file to retry safely.
    const args: StartBuildVideoArgs = JSON.parse(await readFile(value!, 'utf8'));
    if (typeof args.job_id !== 'string' || !args.job_id) throw new Error('Persist a job_id in the request before starting');
    console.error(`job_id=${args.job_id}; on timeout use status/watch with this ID`);
    const job = jobFrom(await client.call('build_video', { ...args, wait_ms: 0 }));
    report(job);
    if (['failed', 'cancelled', 'interrupted'].includes(job.phase)) process.exitCode = 1;
  } else {
    let job: BuildVideoJob;
    do {
      job = jobFrom(await client.call(command === 'cancel' ? 'cancel_build_video' : 'get_build_video',
        command === 'cancel' ? { job_id: value } : { job_id: value, wait_ms: command === 'watch' ? 30_000 : 0 }));
      report(job);
    } while (command === 'watch' && ['queued', 'running', 'cancelling'].includes(job.phase));
    if (['failed', 'cancelled', 'interrupted'].includes(job.phase)) process.exitCode = 1;
  }
} catch (error) {
  console.error(error instanceof Error ? error.message : String(error));
  process.exitCode = 1;
} finally {
  await client.close(); // Closing the client does not cancel the host's production job.
}
