import { spawn, type ChildProcessWithoutNullStreams } from 'node:child_process';
import { createInterface, type Interface } from 'node:readline';

type Pending = { resolve: (value: unknown) => void; reject: (error: Error) => void; timer: NodeJS.Timeout };

/** Keep machine-readable host errors available without parsing display text. */
export class McpToolError extends Error {
  readonly code: string | undefined;
  readonly response: unknown;
  constructor(response: unknown) {
    super(JSON.stringify(response));
    this.name = 'McpToolError';
    this.response = response;
    this.code = response !== null && typeof response === 'object' && 'code' in response
      && typeof response.code === 'string' ? response.code : undefined;
  }
}

/** Minimal local stdio transport. Project changes always go through the MCP host. */
export class McpClient {
  private child: ChildProcessWithoutNullStreams;
  private lines: Interface;
  private pending = new Map<number, Pending>();
  private sequence = 0;
  private failure: Error | null = null;
  private stderr = '';
  private closed: Promise<void>;

  constructor(binary: string, args: string[] = []) {
    this.child = spawn(binary, args, { stdio: ['pipe', 'pipe', 'pipe'] });
    this.closed = new Promise(resolve => this.child.once('close', () => resolve()));
    this.child.stderr.setEncoding('utf8').on('data', (chunk: string) => {
      this.stderr = (this.stderr + chunk).slice(-8192);
    });
    this.child.once('error', error => this.fail(error));
    this.child.once('close', (code, signal) => {
      this.fail(new Error(`MCP closed (${code ?? signal}): ${this.stderr}`));
    });
    this.child.stdin.on('error', error => this.fail(error));
    this.lines = createInterface({ input: this.child.stdout });
    this.lines.on('line', line => {
      try {
        const message = JSON.parse(line);
        if (!message || typeof message !== 'object' || message.jsonrpc !== '2.0') throw new Error('Invalid MCP response');
        if (message.id === undefined) return; // Notifications do not complete a request.
        const request = this.pending.get(message.id);
        if (!request) return;
        const hasResult = Object.hasOwn(message, 'result');
        const hasError = Object.hasOwn(message, 'error');
        if (hasResult === hasError || (hasError && (!message.error
          || !Number.isInteger(message.error.code) || typeof message.error.message !== 'string'))) {
          throw new Error('Invalid MCP response envelope');
        }
        this.pending.delete(message.id); clearTimeout(request.timer);
        if (hasError) request.reject(new Error(JSON.stringify(message.error)));
        else request.resolve(message.result);
      } catch (error) {
        this.fail(error instanceof Error ? error : new Error(String(error)));
        this.child.kill();
      }
    });
  }

  private fail(error: Error) {
    this.failure ??= error;
    for (const request of this.pending.values()) {
      clearTimeout(request.timer); request.reject(error);
    }
    this.pending.clear();
  }

  async initialize() {
    const result = await this.request('initialize', {
      protocolVersion: '2025-11-25', capabilities: {},
      clientInfo: { name: 'next-production-example', version: '1.0' },
    }) as { serverInfo?: { name?: string } };
    if (result.serverInfo?.name !== 'sungeki-mcp') throw new Error('Unexpected MCP server');
    this.child.stdin.write(JSON.stringify({ jsonrpc: '2.0', method: 'notifications/initialized' }) + '\n');
  }

  request(method: string, params: unknown = {}, timeoutMs = 45_000): Promise<unknown> {
    if (this.failure) return Promise.reject(this.failure);
    const id = ++this.sequence;
    let payload: string;
    try { payload = JSON.stringify({ jsonrpc: '2.0', id, method, params }) + '\n'; }
    catch (error) { return Promise.reject(error); }
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => {
        this.pending.delete(id);
        reject(new Error('MCP response timed out. Track the same job ID; do not start a new job.'));
      }, timeoutMs);
      this.pending.set(id, { resolve, reject, timer });
      this.child.stdin.write(payload);
    });
  }

  async call(name: string, args: unknown): Promise<unknown> {
    const result = await this.request('tools/call', { name, arguments: args }) as {
      isError?: boolean; structuredContent?: unknown; content?: unknown;
    };
    if (result.isError) throw new McpToolError(result.structuredContent ?? result.content);
    if (result.structuredContent === undefined) throw new Error('Missing structured MCP result');
    return result.structuredContent;
  }

  async close() {
    this.child.stdin.end();
    const timer = setTimeout(() => this.child.kill(), 2000);
    try { await this.closed; }
    finally { clearTimeout(timer); this.lines.close(); }
  }
}
