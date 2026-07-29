import { ConflictResolver } from "./conflict-resolver";
import { OperationQueue } from "./queue";
import { ReadCache } from "./cache";
import { NetworkStatusMonitor } from "./network-status";
import { OfflineState, OfflineConfig, DEFAULT_OFFLINE_CONFIG } from "./types";
import { QueuedOperation } from "./types";
import { OfflineStorage } from "./storage";

type SyncListener = (state: OfflineState) => void;

export type OperationExecutor = (op: QueuedOperation) => Promise<boolean>;

export class SyncEngine {
  private queue: OperationQueue;
  private cache: ReadCache;
  private networkMonitor: NetworkStatusMonitor;
  private conflictResolver: ConflictResolver;
  private config: OfflineConfig;
  private storage: OfflineStorage;
  private executor: OperationExecutor | null = null;
  private listeners = new Set<SyncListener>();
  private _syncInProgress = false;
  private intervalId: ReturnType<typeof setInterval> | null = null;
  private running = false;

  constructor(
    queue: OperationQueue,
    cache: ReadCache,
    networkMonitor: NetworkStatusMonitor,
    storage: OfflineStorage,
    config: OfflineConfig = DEFAULT_OFFLINE_CONFIG,
  ) {
    this.queue = queue;
    this.cache = cache;
    this.networkMonitor = networkMonitor;
    this.storage = storage;
    this.config = config;
    this.conflictResolver = new ConflictResolver(config);
  }

  setExecutor(executor: OperationExecutor): void {
    this.executor = executor;
  }

  subscribe(listener: SyncListener): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }

  getState(): OfflineState {
    return {
      isOnline: this.networkMonitor.isOnline(),
      queueLength: 0,
      lastSyncTimestamp: null,
      syncInProgress: this._syncInProgress,
    };
  }

  async start(): Promise<void> {
    this.running = true;
    this.networkMonitor.subscribe(async (isOnline) => {
      this.notifyListeners();
      if (isOnline) {
        await this.sync();
      }
    });
    this.networkMonitor.start();
    this.intervalId = setInterval(async () => {
      if (this.networkMonitor.isOnline()) {
        await this.sync();
      }
    }, this.config.syncIntervalMs);
    if (this.networkMonitor.isOnline()) {
      await this.sync();
    }
  }

  stop(): void {
    this.running = false;
    this.networkMonitor.stop();
    if (this.intervalId !== null) {
      clearInterval(this.intervalId);
      this.intervalId = null;
    }
  }

  async sync(): Promise<void> {
    if (this._syncInProgress || !this.executor) return;
    this._syncInProgress = true;
    this.notifyListeners();

    try {
      const pending = await this.queue.listPending();
      if (pending.length === 0) {
        await this.storage.setItem(
          "offline:last_sync",
          Date.now().toString(),
        );
        return;
      }

      const resolved = this.conflictResolver.resolve(pending);

      for (const op of resolved) {
        if (!this.networkMonitor.isOnline()) break;

        op.status = "syncing";
        await this.queue.update(op);

        const success = await this.executor(op);

        if (success) {
          await this.queue.remove(op.id);
        } else {
          op.retryCount += 1;
          op.status = op.retryCount >= this.config.maxRetries ? "failed" : "failed";
          op.lastError = `Sync failed after ${op.retryCount} attempt(s)`;
          await this.queue.update(op);

          if (op.retryCount < this.config.maxRetries) {
            const delay = this.config.retryBaseDelayMs * Math.pow(2, op.retryCount - 1);
            await new Promise((resolve) => setTimeout(resolve, delay));
          }
        }
      }

      await this.storage.setItem(
        "offline:last_sync",
        Date.now().toString(),
      );
    } finally {
      this._syncInProgress = false;
      this.notifyListeners();
    }
  }

  async getLastSyncTimestamp(): Promise<number | null> {
    const raw = await this.storage.getItem("offline:last_sync");
    return raw ? parseInt(raw, 10) : null;
  }

  private notifyListeners(): void {
    const state: OfflineState = {
      isOnline: this.networkMonitor.isOnline(),
      queueLength: 0,
      lastSyncTimestamp: null,
      syncInProgress: this._syncInProgress,
    };
    this.queue.size().then((len) => {
      state.queueLength = len;
    });
    this.getLastSyncTimestamp().then((ts) => {
      state.lastSyncTimestamp = ts;
    });
    for (const listener of this.listeners) {
      listener(state);
    }
  }
}
