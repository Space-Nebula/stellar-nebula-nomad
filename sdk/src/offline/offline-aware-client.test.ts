import { OfflineAwareClient } from "./offline-aware-client";
import { OperationQueue } from "./queue";
import { ReadCache } from "./cache";
import { SyncEngine } from "./sync-engine";
import { DefaultNetworkStatusMonitor } from "./network-status";
import { InMemoryStorage } from "./storage";
import { StellarNebulaClient } from "../client";

jest.mock("../client");

function createMockClient(): jest.Mocked<StellarNebulaClient> {
  return {
    mintShip: jest.fn(),
    scanNebula: jest.fn(),
    harvestResources: jest.fn(),
    getShip: jest.fn(),
    getResourceBalance: jest.fn(),
    stakeResources: jest.fn(),
    claimYield: jest.fn(),
  } as any;
}

describe("OfflineAwareClient", () => {
  let mockClient: jest.Mocked<StellarNebulaClient>;
  let storage: InMemoryStorage;
  let queue: OperationQueue;
  let cache: ReadCache;
  let networkMonitor: DefaultNetworkStatusMonitor;
  let syncEngine: SyncEngine;
  let offlineClient: OfflineAwareClient;

  beforeEach(() => {
    mockClient = createMockClient();
    storage = new InMemoryStorage();
    queue = new OperationQueue(storage);
    cache = new ReadCache(storage);
    networkMonitor = new DefaultNetworkStatusMonitor();
    syncEngine = new SyncEngine(queue, cache, networkMonitor, storage);
    offlineClient = new OfflineAwareClient(
      mockClient,
      queue,
      cache,
      syncEngine,
    );
  });

  it("queues write operations when offline", async () => {
    jest.spyOn(networkMonitor, "isOnline").mockReturnValue(false);
    const mockSigner = {
      getPublicKey: jest.fn().mockResolvedValue("GABC"),
      signTransaction: jest.fn().mockResolvedValue(""),
    };

    const result = await offlineClient.mintShip(mockSigner, "GABC", 0);

    expect(result.success).toBe(true);
    expect(result.txHash).toBe("queued");
    expect(mockClient.mintShip).not.toHaveBeenCalled();
    expect(await queue.size()).toBe(1);
  });

  it("calls through when online", async () => {
    mockClient.getShip.mockResolvedValue({
      id: BigInt(1),
      owner: "GABC",
      shipType: 0 as any,
      rarity: 0 as any,
      stats: { speed: 10, cargo: 20, weapons: 5, shields: 5 },
    });

    const result = await offlineClient.getShip(BigInt(1));
    expect(result).not.toBeNull();
    expect(mockClient.getShip).toHaveBeenCalledWith(BigInt(1));
  });

  it("returns cached data for read calls when offline", async () => {
    mockClient.getShip.mockResolvedValue({
      id: BigInt(1),
      owner: "GABC",
      shipType: 0 as any,
      rarity: 0 as any,
      stats: { speed: 10, cargo: 20, weapons: 5, shields: 5 },
    });

    await offlineClient.getShip(BigInt(1));
    expect(mockClient.getShip).toHaveBeenCalledTimes(1);

    jest.spyOn(networkMonitor, "isOnline").mockReturnValue(false);

    const cached = await offlineClient.getShip(BigInt(1));
    expect(cached).not.toBeNull();
    expect(cached!.id).toBe(BigInt(1));
    expect(mockClient.getShip).toHaveBeenCalledTimes(1);
  });
});
