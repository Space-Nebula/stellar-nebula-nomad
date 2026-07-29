import { CacheEntry, OfflineConfig, DEFAULT_OFFLINE_CONFIG } from "./types";
import { OfflineStorage } from "./storage";

export class ReadCache {
  private storage: OfflineStorage;
  private config: OfflineConfig;

  constructor(
    storage: OfflineStorage,
    config: OfflineConfig = DEFAULT_OFFLINE_CONFIG,
  ) {
    this.storage = storage;
    this.config = config;
  }

  async get<T>(key: string): Promise<T | null> {
    const raw = await this.storage.getItem(this.cacheKey(key));
    if (!raw) return null;

    try {
      const entry: CacheEntry<T> = JSON.parse(raw, this.reviver);
      const now = Date.now();
      if (now - entry.timestamp > entry.ttl) {
        await this.storage.removeItem(this.cacheKey(key));
        return null;
      }
      return entry.data;
    } catch {
      return null;
    }
  }

  async set<T>(
    key: string,
    data: T,
    ttl?: number,
  ): Promise<void> {
    const entry: CacheEntry<T> = {
      data,
      timestamp: Date.now(),
      ttl: ttl ?? this.config.cacheDefaultTtlMs,
    };
    await this.storage.setItem(
      this.cacheKey(key),
      JSON.stringify(entry, this.replacer),
    );
  }

  async invalidate(key: string): Promise<void> {
    await this.storage.removeItem(this.cacheKey(key));
  }

  async invalidateAll(): Promise<void> {
    const keys = await this.storage.getKeys("read_cache:");
    for (const key of keys) {
      await this.storage.removeItem(key);
    }
  }

  private cacheKey(key: string): string {
    return `read_cache:${key}`;
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
