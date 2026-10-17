import { spawn } from 'node:child_process';
import { createInterface } from 'node:readline';
import assert from 'node:assert/strict';
import expectedTools from './mcp-tools.json' with { type: 'json' };

const child = spawn('./target/release/sungeki-mcp', [], { stdio: ['pipe', 'pipe', 'pipe'] });
const waiting = new Map();
let stderr = '';
child.stderr.setEncoding('utf8').on('data', (data) => { stderr += data; });
const output = createInterface({ input: child.stdout });
let protocolError;
output.on('line', (line) => {
  try {
    const message = JSON.parse(line);
    assert.equal(message.jsonrpc, '2.0');
    waiting.get(message.id)?.(message);
    waiting.delete(message.id);
  } catch (error) {
    protocolError = error;
    child.kill();
  }
});
const exited = new Promise((resolve, reject) => {
  child.on('error', reject);
  child.on('exit', (code) => resolve(code));
});
const timer = setTimeout(() => child.kill(), 10000);
function request(id, method, params) {
  const response = new Promise((resolve) => waiting.set(id, resolve));
  child.stdin.write(JSON.stringify({ jsonrpc: '2.0', id, method, params }) + '\n');
  return Promise.race([response, exited.then(() => { throw new Error(`MCP終了: ${stderr}`); })]);
}
try {
  const initialized = await request(1, 'initialize', {
    protocolVersion: '2025-11-25', capabilities: {},
    clientInfo: { name: 'next-smoke-check', version: '0.1.0' },
  });
  assert.equal(initialized.result.serverInfo.name, 'sungeki-mcp');
  assert.ok(initialized.result.capabilities.tools);
  child.stdin.write(JSON.stringify({ jsonrpc: '2.0', method: 'notifications/initialized' }) + '\n');
  const listed = await request(4, 'tools/list');
  const tools = listed.result.tools;
  assert.deepEqual(tools.map(tool => tool.name).sort(), expectedTools);
  assert.equal(new Set(tools.map(tool => tool.name)).size, tools.length);
  for (const tool of tools) {
    assert.ok(tool.description?.trim(), `${tool.name}: description missing`);
    assert.equal(tool.inputSchema?.type, 'object', `${tool.name}: object schema required`);
  }
  assert.deepEqual((await request(2, 'ping')).result, {});
  const unsupported = await request(3, 'next/unknown');
  assert.equal(typeof unsupported.error.code, 'number');
  child.stdin.end();
  assert.equal(await exited, 0);
  if (protocolError) throw protocolError;
  console.log('stdio MCP: initialize / ping / 不正要求 / EOF終了 合格');
} finally {
  clearTimeout(timer);
  output.close();
  if (child.exitCode === null) child.kill();
}
