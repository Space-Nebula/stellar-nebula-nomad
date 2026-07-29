export interface QueuedOperation {
  id: string;
  method: string;
  args: any[];
  callerSerialized: string;
  timestamp: number;
  status: "pending" | "syncing" | "completed" | "failed";
  retryCount: number;
  lastError?: string;
}

export interface OfflineState {
  isOnline: boolean;
  queueLength: number;
  lastSyncTimestamp: number | null;
  syncInProgress: boolean;
}

export interface CacheEntry<T = any> {
  data: T;
  timestamp: number;
  ttl: number;
}

export type ConflictStrategy = "last-write-wins" | "server-priority";

export interface OfflineConfig {
  conflictStrategy: ConflictStrategy;
  maxRetries: number;
  retryBaseDelayMs: number;
  cacheDefaultTtlMs: number;
  syncIntervalMs: number;
}

export const DEFAULT_OFFLINE_CONFIG: OfflineConfig = {
  conflictStrategy: "last-write-wins",
  maxRetries: 3,
  retryBaseDelayMs: 1000,
  cacheDefaultTtlMs: 5 * 60 * 1000,
  syncIntervalMs: 30 * 1000,
};
