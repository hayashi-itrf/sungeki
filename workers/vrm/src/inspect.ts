import { parseArgs } from 'node:util';
import { readGlb } from './inputs.ts';

// Metadata-only entry point: does not load a browser or create GPU resources.
try {
  const { values } = parseArgs({ options: { path: { type: 'string' }, kind: { type: 'string' } } });
  if (!values.path || (values.kind !== 'model' && values.kind !== 'motion')) {
    throw new Error('--path <absolute file> --kind <model|motion> が必要です');
  }
  const { manifest } = await readGlb(values.path, values.kind);
  process.stdout.write(JSON.stringify({ version: 1, ...manifest }) + '\n');
} catch (error) {
  process.stderr.write(String(error) + '\n');
  process.exitCode = 1;
}
