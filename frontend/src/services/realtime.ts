import {
  ConnectionStatus,
  EventHandler,
  WebSocketClient,
  WebSocketClientOptions,
  WebSocketEvent,
} from './websocket';

export const REALTIME_CHANNELS = {
  SYSTEM: 'system',
  NOTIFICATIONS: 'notifications',
  MARKET_PRICES: 'market_prices',
  LEADERBOARD: 'leaderboard',
  GUILD: 'guild',
  INVENTORY: 'inventory',
  QUESTS: 'quests',
} as const;

export type RealtimeChannel =
  (typeof REALTIME_CHANNELS)[keyof typeof REALTIME_CHANNELS];

export interface RealtimeNotification {
  id: string;
  type: string;
  channel: string;
  message: string;
  severity: 'info' | 'warning' | 'error' | 'success';
  timestamp: number;
  payload: unknown;
}

export interface RealtimeSnapshot<T = Record<string, unknown>> {
  state: T;
  revision: number;
}

export interface RealtimeDelta<T = unknown> {
  key: string;
  value: T;
  revision?: number;
}

export type RealtimeStatusHandler = (status: ConnectionStatus) => void;
export type RealtimeNotificationHandler = (
  notification: RealtimeNotification,
) => void;
export type RealtimeStateHandler = (state: ReadonlyMap<string, unknown>) => void;

export interface RealtimeClientOptions
  extends Partial<WebSocketClientOptions> {
  url?: string;
}

export function resolveRealtimeUrl(configured?: string): string {
  if (configured) {
    return configured;
  }

  if (typeof window === 'undefined') {
    return 'ws://localhost:8080/ws';
  }

  const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
  return `${protocol}//${window.location.host}/ws`;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null;
}

function notificationSeverity(value: unknown): RealtimeNotification['severity'] {
  return value === 'warning' ||
    value === 'error' ||
    value === 'success'
    ? value
    : 'info';
}

export function toRealtimeNotification(
  event: WebSocketEvent,
): RealtimeNotification | null {
  if (
    event.type !== 'notification' &&
    event.channel !== REALTIME_CHANNELS.NOTIFICATIONS
  ) {
    return null;
  }

  const payload = isRecord(event.payload) ? event.payload : {};
  const message =
    typeof payload.message === 'string'
      ? payload.message
      : typeof payload.title === 'string'
        ? payload.title
        : event.type;

  return {
    id:
      typeof payload.id === 'string'
        ? payload.id
        : `${event.channel}-${event.sequence}`,
    type:
      typeof payload.notificationType === 'string'
        ? payload.notificationType
        : event.type,
    channel: event.channel,
    message,
    severity: notificationSeverity(payload.severity),
    timestamp:
      typeof payload.timestamp === 'number' ? payload.timestamp : event.timestamp,
    payload: event.payload,
  };
}

export class RealtimeClient {
  private readonly socket: WebSocketClient;
  private readonly channels = new Set<string>();
  private readonly state = new Map<string, unknown>();
  private readonly statusHandlers = new Set<RealtimeStatusHandler>();
  private readonly notificationHandlers = new Set<RealtimeNotificationHandler>();
  private readonly stateHandlers = new Set<RealtimeStateHandler>();
  private status: ConnectionStatus = 'disconnected';
  private revision = 0;

  constructor(options: RealtimeClientOptions = {}) {
    const { url, ...clientOptions } = options;
    this.socket = new WebSocketClient({
      ...clientOptions,
      url: resolveRealtimeUrl(url),
    });

    this.socket.subscribe('*', (event) => this.handleEvent(event));
    this.socket.onStatusChange((status) => {
      this.status = status;
      for (const handler of this.statusHandlers) {
        handler(status);
      }
      if (status === 'connected') {
        this.requestStateSync();
      }
    });
  }

  public connect(): void {
    this.socket.connect();
  }

  public disconnect(): void {
    this.socket.disconnect();
  }

  public getStatus(): ConnectionStatus {
    return this.status;
  }

  public onStatusChange(handler: RealtimeStatusHandler): () => void {
    this.statusHandlers.add(handler);
    handler(this.status);
    return () => this.statusHandlers.delete(handler);
  }

  public subscribe<T = unknown>(
    channel: string,
    handler: EventHandler<T>,
  ): () => void {
    this.channels.add(channel);
    return this.socket.subscribe(channel, handler);
  }

  public onNotification(
    handler: RealtimeNotificationHandler,
  ): () => void {
    this.notificationHandlers.add(handler);
    return () => this.notificationHandlers.delete(handler);
  }

  public onStateChange(handler: RealtimeStateHandler): () => void {
    this.stateHandlers.add(handler);
    handler(this.state);
    return () => this.stateHandlers.delete(handler);
  }

  public getState<T = unknown>(key: string): T | undefined {
    return this.state.get(key) as T | undefined;
  }

  public getRevision(): number {
    return this.revision;
  }

  public requestStateSync(): void {
    this.socket.send({
      type: 'snapshot',
      channels: Array.from(this.channels),
    });
  }

  public send(payload: Record<string, unknown>): void {
    this.socket.send(payload);
  }

  private handleEvent(event: WebSocketEvent): void {
    const notification = toRealtimeNotification(event);
    if (notification) {
      for (const handler of this.notificationHandlers) {
        handler(notification);
      }
    }

    if (event.type === 'state_snapshot') {
      this.applySnapshot(event.payload);
    } else if (event.type === 'state_delta') {
      this.applyDelta(event.payload);
    }
  }

  private applySnapshot(payload: unknown): void {
    if (!isRecord(payload)) {
      return;
    }

    const nextState = isRecord(payload.state)
      ? payload.state
      : isRecord(payload.snapshot)
        ? payload.snapshot
        : payload;
    this.state.clear();
    for (const [key, value] of Object.entries(nextState)) {
      if (key !== 'revision' && key !== 'state' && key !== 'snapshot') {
        this.state.set(key, value);
      }
    }
    if (typeof payload.revision === 'number') {
      this.revision = payload.revision;
    }
    this.emitState();
  }

  private applyDelta(payload: unknown): void {
    if (!isRecord(payload) || typeof payload.key !== 'string') {
      return;
    }

    this.state.set(payload.key, payload.value);
    if (typeof payload.revision === 'number') {
      this.revision = Math.max(this.revision, payload.revision);
    }
    this.emitState();
  }

  private emitState(): void {
    for (const handler of this.stateHandlers) {
      handler(this.state);
    }
  }
}
