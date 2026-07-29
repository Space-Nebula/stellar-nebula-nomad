import { StellarNebulaClient } from "../client";
import { Ship, TxResult, NebulaLayout, ShipType, ResourceType, Signer } from "../types";
import { toSigner } from "../signer";
import { Keypair } from "@stellar/stellar-sdk";
import { OperationQueue } from "./queue";
import { ReadCache } from "./cache";
import { SyncEngine } from "./sync-engine";
import { QueuedOperation } from "./types";

export class OfflineAwareClient {
  private client: StellarNebulaClient;
  private queue: OperationQueue;
  private cache: ReadCache;
  private syncEngine: SyncEngine;

  constructor(
    client: StellarNebulaClient,
    queue: OperationQueue,
    cache: ReadCache,
    syncEngine: SyncEngine,
  ) {
    this.client = client;
    this.queue = queue;
    this.cache = cache;
    this.syncEngine = syncEngine;
    this.setupSyncExecutor();
  }

  async mintShip(
    caller: Keypair | Signer,
    owner: string,
    shipType: ShipType,
    options?: { fee?: string; timeout?: number },
  ): Promise<TxResult<bigint>> {
    if (this.syncEngine.getState().isOnline) {
      return this.client.mintShip(caller, owner, shipType, options);
    }
    const serialized = await this.serializeCaller(caller);
    await this.queue.enqueue({
      method: "mintShip",
      args: [owner, shipType, options],
      callerSerialized: serialized,
    });
    return { success: true, result: BigInt(0), txHash: "queued" };
  }

  async scanNebula(
    caller: Keypair | Signer,
    nebulaId: bigint,
    options?: { fee?: string; timeout?: number },
  ): Promise<TxResult<NebulaLayout>> {
    if (this.syncEngine.getState().isOnline) {
      return this.client.scanNebula(caller, nebulaId, options);
    }
    const serialized = await this.serializeCaller(caller);
    await this.queue.enqueue({
      method: "scanNebula",
      args: [nebulaId, options],
      callerSerialized: serialized,
    });
    return { success: true, txHash: "queued" };
  }

  async harvestResources(
    caller: Keypair | Signer,
    shipId: bigint,
    resourceType: ResourceType,
    options?: { fee?: string; timeout?: number },
  ): Promise<TxResult<bigint>> {
    if (this.syncEngine.getState().isOnline) {
      return this.client.harvestResources(caller, shipId, resourceType, options);
    }
    const serialized = await this.serializeCaller(caller);
    await this.queue.enqueue({
      method: "harvestResources",
      args: [shipId, resourceType, options],
      callerSerialized: serialized,
    });
    return { success: true, result: BigInt(0), txHash: "queued" };
  }

  async getShip(shipId: bigint): Promise<Ship | null> {
    const cacheKey = `ship:${shipId}`;
    const cached = await this.cache.get<Ship>(cacheKey);
    if (cached !== null) return cached;

    if (this.syncEngine.getState().isOnline) {
      const result = await this.client.getShip(shipId);
      if (result) {
        await this.cache.set(cacheKey, result);
      }
      return result;
    }
    return cached;
  }

  async getResourceBalance(
    address: string,
    resourceType: ResourceType,
  ): Promise<bigint> {
    const cacheKey = `resource_balance:${address}:${resourceType}`;
    const cached = await this.cache.get<bigint>(cacheKey);
    if (cached !== null) return cached;

    if (this.syncEngine.getState().isOnline) {
      const result = await this.client.getResourceBalance(address, resourceType);
      await this.cache.set(cacheKey, result);
      return result;
    }
    return cached ?? BigInt(0);
  }

  async stakeResources(
    caller: Keypair | Signer,
    resourceType: ResourceType,
    amount: bigint,
    duration: number,
    options?: { fee?: string; timeout?: number },
  ): Promise<TxResult<void>> {
    if (this.syncEngine.getState().isOnline) {
      return this.client.stakeResources(caller, resourceType, amount, duration, options);
    }
    const serialized = await this.serializeCaller(caller);
    await this.queue.enqueue({
      method: "stakeResources",
      args: [resourceType, amount, duration, options],
      callerSerialized: serialized,
    });
    return { success: true, txHash: "queued" };
  }

  async claimYield(
    caller: Keypair | Signer,
    stakeId: bigint,
    options?: { fee?: string; timeout?: number },
  ): Promise<TxResult<bigint>> {
    if (this.syncEngine.getState().isOnline) {
      return this.client.claimYield(caller, stakeId, options);
    }
    const serialized = await this.serializeCaller(caller);
    await this.queue.enqueue({
      method: "claimYield",
      args: [stakeId, options],
      callerSerialized: serialized,
    });
    return { success: true, result: BigInt(0), txHash: "queued" };
  }

  private setupSyncExecutor(): void {
    this.syncEngine.setExecutor(async (op: QueuedOperation) => {
      try {
        const caller = await this.deserializeCaller(op.callerSerialized);
        const result = await this.callOriginalMethod(
          op.method,
          caller,
          ...op.args,
        );
        return result.success;
      } catch {
        return false;
      }
    });
  }

  private async callOriginalMethod(
    method: string,
    caller: Keypair | Signer,
    ...args: any[]
  ): Promise<TxResult> {
    switch (method) {
      case "mintShip":
        return this.client.mintShip(caller, args[0], args[1], args[2]);
      case "scanNebula":
        return this.client.scanNebula(caller, args[0], args[1]);
      case "harvestResources":
        return this.client.harvestResources(caller, args[0], args[1], args[2]);
      case "stakeResources":
        return this.client.stakeResources(caller, args[0], args[1], args[2], args[3]);
      case "claimYield":
        return this.client.claimYield(caller, args[0], args[1]);
      default:
        throw new Error(`Unknown method: ${method}`);
    }
  }

  private async serializeCaller(
    caller: Keypair | Signer,
  ): Promise<string> {
    const signer = toSigner(caller);
    const publicKey = await signer.getPublicKey();
    return `keypair:${publicKey}`;
  }

  private async deserializeCaller(
    serialized: string,
  ): Promise<Signer> {
    if (serialized.startsWith("keypair:")) {
      const publicKey = serialized.slice("keypair:".length);
      return toSigner(Keypair.fromPublicKey(publicKey));
    }
    throw new Error("Unsupported caller serialization");
  }
}
