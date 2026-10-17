import { execFile } from 'node:child_process';
import { promisify } from 'node:util';

export interface OwnedProcess { pid: number; parent: number; command: string }

export async function profileProcesses(profile: string, tracked: OwnedProcess[] = []) {
  const { stdout } = await promisify(execFile)('ps', ['-axo', 'pid=,ppid=,command=']);
  const rows = stdout.split('\n').flatMap((line) => {
    const match = line.trim().match(/^(\d+)\s+(\d+)\s+(.+)$/);
    return match ? [{ pid: Number(match[1]), parent: Number(match[2]), command: match[3] }] : [];
  });
  const selected = new Set(rows.filter((row) => row.command.includes(`--user-data-dir=${profile}`)
    || tracked.some((known) => known.pid === row.pid && known.command === row.command)).map((row) => row.pid));
  let added = true;
  while (added) {
    added = false;
    for (const row of rows) {
      if (selected.has(row.parent) && !selected.has(row.pid)) { selected.add(row.pid); added = true; }
    }
  }
  return rows.filter((row) => selected.has(row.pid));
}
