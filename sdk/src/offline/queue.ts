import { QueuedOperation, OfflineConfig, DEFAULT_OFFLINE_CONFIG } from "./types";
import { OfflineStorage } from "./storage";

export class OperationQueue {
  private storage: OfflineStorage;
  private config: OfflineConfig;

  constructor(
    storage: OfflineStorage,
    config: OfflineConfig = DEFAULT_OFFLINE_CONFIG,
  ) {
    this.storage = storage;
    this.config = config;
  }

  async enqueue(operation: Omit<QueuedOperation, "id" | "timestamp" | "status" | "retryCount">): Promise<QueuedOperation> {
    const op: QueuedOperation = {
      ...operation,
      id: this.generateId(),
      timestamp: Date.now(),
      status: "pending",
      retryCount: 0,
    };
    await this.storage.setItem(
      this.operationKey(op.id),
      JSON.stringify(op, this.replacer),
    );
    return op;
  }

  async dequeue(): Promise<QueuedOperation | null> {
    const all = await this.listPending();
    if (all.length === 0) return null;
    all.sort((a, b) => a.timestamp - b.timestamp);
    const op = all[0];
    op.status = "syncing";
    await this.update(op);
    return op;
  }

  async peek(): Promise<QueuedOperation | null> {
    const all = await this.listPending();
    if (all.length === 0) return null;
    all.sort((a, b) => a.timestamp - b.timestamp);
    return all[0];
  }

  async update(operation: QueuedOperation): Promise<void> {
    await this.storage.setItem(
      this.operationKey(operation.id),
      JSON.stringify(operation, this.replacer),
    );
  }

  async remove(id: string): Promise<void> {
    await this.storage.removeItem(this.operationKey(id));
  }

  async listPending(): Promise<QueuedOperation[]> {
    const keys = await this.storage.getKeys("offline_op:");
    const ops: QueuedOperation[] = [];
    for (const key of keys) {
      const raw = await this.storage.getItem(key);
      if (!raw) continue;
      try {
        const op = JSON.parse(raw, this.reviver) as QueuedOperation;
        if (op.status === "pending" || op.status === "failed") {
          if (op.status === "failed" && op.retryCount >= this.config.maxRetries) {
            continue;
          }
          ops.push(op);
        }
      } catch {
        continue;
      }
    }
    return ops;
  }

  async listAll(): Promise<QueuedOperation[]> {
    const keys = await this.storage.getKeys("offline_op:");
    const ops: QueuedOperation[] = [];
    for (const key of keys) {
      const raw = await this.storage.getItem(key);
      if (!raw) continue;
      try {
        ops.push(JSON.parse(raw, this.reviver) as QueuedOperation);
      } catch {
        continue;
      }
    }
    return ops;
  }

  async clear(): Promise<void> {
    const keys = await this.storage.getKeys("offline_op:");
    for (const key of keys) {
      await this.storage.removeItem(key);
    }
  }

  async size(): Promise<number> {
    const pending = await this.listPending();
    return pending.length;
  }

  private generateId(): string {
    return `${Date.now()}_${Math.random().toString(36).slice(2, 9)}`;
  }

  private operationKey(id: string): string {
    return `offline_op:${id}`;
  }

  private replacer(_key: string, value: any): any {
    if (typeof value === "bigint") {
      return { __type: "bigint", value: value.toString() };
    }
    return value;
  }

  private reviver(_key: string, value: any): any {
    if (value && typeof value === "object" && value.__type === "bigint") {
      return BigInt(value.value);
    }
    return value;
  }
}
