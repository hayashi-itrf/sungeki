import { test } from 'node:test';
import assert from 'node:assert/strict';
import { McpClient, McpToolError } from './client.ts';

function server(handler: string) {
  return new McpClient(process.execPath, ['--input-type=module', '-e', `
    import { createInterface } from 'node:readline';
    const lines = createInterface({ input: process.stdin });
    lines.on('line', line => {
      const message = JSON.parse(line);
      const reply = result => console.log(JSON.stringify({jsonrpc:'2.0',id:message.id,result}));
      ${handler}
    });
  `]);
}
test('concurrent requests retain IDs despite reversed responses and a notification', async () => {
  const c = server(`
    console.log(JSON.stringify({jsonrpc:'2.0',method:'notifications/progress'}));
    setTimeout(() => reply(message.method), message.method === 'first' ? 30 : 0);
  `);
  try { assert.deepEqual(await Promise.all([c.request('first'), c.request('second')]), ['first', 'second']); }
  finally { await c.close(); }
});
test('MCP structured errors preserve the common error code', async () => {
  const c = server(`reply({isError:true, structuredContent:{code:'VALIDATION_FAILED',message:'invalid input'}});`);
  try { await assert.rejects(c.call('build_video', {}), error => {
    assert.ok(error instanceof McpToolError);
    assert.equal(error.code, 'VALIDATION_FAILED');
    assert.deepEqual(error.response, {code:'VALIDATION_FAILED',message:'invalid input'});
    return true;
  }); }
  finally { await c.close(); }
});
test('response timeout does not send another start or a cancellation', async () => {
  const c = server(`
    globalThis.calls ??= []; globalThis.calls.push(message.method);
    if (message.method === 'ping') reply(globalThis.calls);
  `);
  try {
    await assert.rejects(c.request('tools/call', {name:'build_video'}, 50), /same job ID/);
    assert.deepEqual(await c.request('ping'), ['tools/call', 'ping']);
  } finally { await c.close(); }
});
test('premature server exit rejects a pending request', async () => {
  const c = server('process.exit(7);');
  try { await assert.rejects(c.request('ping'), /MCP closed \(7\)/); }
  finally { await c.close(); }
});
test('stdout contamination fails instead of becoming a valid result', async () => {
  const c = server("console.log('debug log on stdout');");
  try { await assert.rejects(c.request('ping')); }
  finally { await c.close(); }
});
test('missing binary reports failure and closes without hanging', async () => {
  const c = new McpClient('/nonexistent-vvm-example-binary');
  try { await assert.rejects(c.request('ping'), /ENOENT/); }
  finally { await c.close(); }
});
test('malformed response envelopes never report success', async () => {
  for (const fields of [{}, {result:'ok',error:{code:-1,message:'failed'}}, {error:null}]) {
    const c = server(`console.log(JSON.stringify({jsonrpc:'2.0',id:message.id,...${JSON.stringify(fields)}}));`);
    try { await assert.rejects(c.request('edit'), /Invalid MCP response envelope/); }
    finally { await c.close(); }
  }
});
test('unserializable input sends nothing and does not prevent subsequent requests', async () => {
  const c = server(`globalThis.calls ??= []; globalThis.calls.push(message.method); reply(globalThis.calls);`);
  try {
    const circular: {self?: unknown} = {}; circular.self = circular;
    await assert.rejects(c.request('edit', circular), /circular/i);
    await assert.rejects(c.request('edit', {value:1n}), /BigInt/i);
    assert.deepEqual(await c.request('ping'), ['ping']);
  } finally { await c.close(); }
});
