import type { HostInfo, SessionRequest, SessionResponse, EventCursor, EventBatch } from './generated';
export type * from './generated';

export interface HostTransport {
  watch(after: EventCursor | null, waitMs: number): Promise<EventBatch>;
  getCapabilities(): Promise<HostInfo>;
  session(request: SessionRequest): Promise<SessionResponse>;
}

export class HostApi {
  constructor(private readonly transport: HostTransport) {}
  session(request: SessionRequest): Promise<SessionResponse> {
    return this.transport.session(request);
  }
  watch(after: EventCursor | null, waitMs: number): Promise<EventBatch> {
    return this.transport.watch(after, waitMs);
  }
  getCapabilities(): Promise<HostInfo> {
    return this.transport.getCapabilities();
  }
}
