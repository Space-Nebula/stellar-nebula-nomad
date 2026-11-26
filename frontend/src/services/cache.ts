export interface CacheOptions {
  ttlMs: number;
  namespace?: string;
  maxEntries?: number;
}

export interface CacheRecord<T> {
  value: T;
  expiresAt: number;
  storedAt: number;
}

const DEFAULT_MAX_ENTRIES = 200;

function storage(): Storage | null {
  try {
    return typeof localStorage === 'undefined' ? null : localStorage;
  } catch {
    return null;
  }
}

function cacheKey(namespace: string, key: string): string {
  return `nebula-cache:${namespace}:${key}`;
}

export class ApplicationCache {
  constructor(private readonly options: CacheOptions) {}

  public get<T>(key: string): T | undefined {
    const store = storage();
    if (!store) {
      return undefined;
    }

    const raw = store.getItem(cacheKey(this.namespace, key));
    if (!raw) {
      return undefined;
    }

    try {
      const record = JSON.parse(raw) as CacheRecord<T>;
      if (record.expiresAt <= Date.now()) {
        store.removeItem(cacheKey(this.namespace, key));
        return undefined;
      }
      return record.value;
    } catch {
      store.removeItem(cacheKey(this.namespace, key));
      return undefined;
    }
  }

  public async getOrLoad<T>(
    key: string,
    loader: () => Promise<T> | T,
  ): Promise<T> {
    const cached = this.get<T>(key);
    if (cached !== undefined) {
      return cached;
    }

    const value = await loader();
    this.set(key, value);
    return value;
  }

  public set<T>(key: string, value: T): void {
    const store = storage();
    if (!store) {
      return;
    }

    const now = Date.now();
    const record: CacheRecord<T> = {
      value,
      storedAt: now,
      expiresAt: now + this.options.ttlMs,
    };
    store.setItem(cacheKey(this.namespace, key), JSON.stringify(record));
    this.evictOldest(store);
  }

  public invalidate(key: string): void {
    storage()?.removeItem(cacheKey(this.namespace, key));
  }

  public clear(): void {
    const store = storage();
    if (!store) {
      return;
    }

    const prefix = `nebula-cache:${this.namespace}:`;
    const keys: string[] = [];
    for (let index = 0; index < store.length; index += 1) {
      const key = store.key(index);
      if (key && key.startsWith(prefix)) {
        keys.push(key);
      }
    }
    for (const key of keys) {
      store.removeItem(key);
    }
  }

  private get namespace(): string {
    return this.options.namespace ?? 'default';
  }

  private evictOldest(store: Storage): void {
    const maxEntries = this.options.maxEntries ?? DEFAULT_MAX_ENTRIES;
    const prefix = `nebula-cache:${this.namespace}:`;
    const entries: Array<{ key: string; storedAt: number }> = [];

    for (let index = 0; index < store.length; index += 1) {
      const key = store.key(index);
      if (!key || !key.startsWith(prefix)) {
        continue;
      }
      const raw = store.getItem(key);
      if (!raw) {
        continue;
      }
      try {
        const record = JSON.parse(raw) as CacheRecord<unknown>;
        entries.push({ key, storedAt: record.storedAt });
      } catch {
        store.removeItem(key);
      }
    }

    if (entries.length <= maxEntries) {
      return;
    }

    entries.sort((left, right) => left.storedAt - right.storedAt);
    for (const entry of entries.slice(0, entries.length - maxEntries)) {
      store.removeItem(entry.key);
    }
  }
}

export const apiResponseCache = new ApplicationCache({
  namespace: 'api-response',
  ttlMs: 60_000,
});

export const frontendStateCache = new ApplicationCache({
  namespace: 'frontend-state',
  ttlMs: 300_000,
});

export const assetCache = new ApplicationCache({
  namespace: 'asset',
  ttlMs: 3_600_000,
});

export function invalidateCaches(namespace?: string): void {
  const caches = [apiResponseCache, frontendStateCache, assetCache];
  for (const cache of caches) {
    if (!namespace || namespace === 'all') {
      cache.clear();
    }
  }
}
