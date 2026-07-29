import { ReadCache } from "./cache";
import { InMemoryStorage } from "./storage";

describe("ReadCache", () => {
  let cache: ReadCache;
  let storage: InMemoryStorage;

  beforeEach(() => {
    storage = new InMemoryStorage();
    cache = new ReadCache(storage, { cacheDefaultTtlMs: 60000 } as any);
  });

  it("stores and retrieves values", async () => {
    await cache.set("test-key", { hello: "world" });
    const result = await cache.get<{ hello: string }>("test-key");
    expect(result).toEqual({ hello: "world" });
  });

  it("returns null for missing keys", async () => {
    const result = await cache.get("nonexistent");
    expect(result).toBeNull();
  });

  it("handles bigint values", async () => {
    await cache.set("bigint-key", BigInt("9999999999999"));
    const result = await cache.get<bigint>("bigint-key");
    expect(result).toBe(BigInt("9999999999999"));
  });

  it("invalidates specific keys", async () => {
    await cache.set("key1", "value1");
    await cache.set("key2", "value2");
    await cache.invalidate("key1");

    expect(await cache.get("key1")).toBeNull();
    expect(await cache.get("key2")).toBe("value2");
  });

  it("invalidates all keys", async () => {
    await cache.set("key1", "value1");
    await cache.set("key2", "value2");
    await cache.invalidateAll();

    expect(await cache.get("key1")).toBeNull();
    expect(await cache.get("key2")).toBeNull();
  });
});
