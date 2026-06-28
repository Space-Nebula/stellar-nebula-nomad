import {
  Contract,
  Server,
  TransactionBuilder,
  Networks,
  Operation,
  Keypair,
  Account,
  BASE_FEE,
} from "@stellar/stellar-sdk";
import {
  ContractConfig,
  TransactionOptions,
  Ship,
  NebulaLayout,
  ResourceBalance,
  TxResult,
  ShipType,
  ResourceType,
} from "./types";

export class StellarNebulaClient {
  private contract: Contract;
  private server: Server;
  private config: ContractConfig;

  constructor(config: ContractConfig) {
    this.config = config;
    this.contract = new Contract(config.contractId);
    this.server = new Server(config.rpcUrl);
  }

  /**
   * Mint a new ship NFT
   */
  async mintShip(
    caller: Keypair,
    owner: string,
    shipType: ShipType,
    options?: TransactionOptions,
  ): Promise<TxResult<bigint>> {
    return this.executeTransaction(
      caller,
      "mint_ship",
      [owner, shipType],
      options,
    );
  }

  /**
   * Scan a nebula and generate layout
   */
  async scanNebula(
    caller: Keypair,
    nebulaId: bigint,
    options?: TransactionOptions,
  ): Promise<TxResult<NebulaLayout>> {
    return this.executeTransaction(caller, "scan_nebula", [nebulaId], options);
  }

  /**
   * Harvest resources from a location
   */
  async harvestResources(
    caller: Keypair,
    shipId: bigint,
    resourceType: ResourceType,
    options?: TransactionOptions,
  ): Promise<TxResult<bigint>> {
    return this.executeTransaction(
      caller,
      "harvest_resources",
      [shipId, resourceType],
      options,
    );
  }

  /**
   * Get ship details by ID
   */
  async getShip(shipId: bigint): Promise<Ship | null> {
    try {
      const result = await this.server.getContractData(
        this.config.contractId,
        this.contract.call("get_ship", shipId),
      );
      return result as Ship;
    } catch (error) {
      return null;
    }
  }

  /**
   * Get resource balance for an address
   */
  async getResourceBalance(
    address: string,
    resourceType: ResourceType,
  ): Promise<bigint> {
    try {
      const result = await this.server.getContractData(
        this.config.contractId,
        this.contract.call("get_resource_balance", address, resourceType),
      );
      return BigInt(result as string);
    } catch (error) {
      return BigInt(0);
    }
  }

  /**
   * Stake resources for yield farming
   */
  async stakeResources(
    caller: Keypair,
    resourceType: ResourceType,
    amount: bigint,
    duration: number,
    options?: TransactionOptions,
  ): Promise<TxResult<void>> {
    return this.executeTransaction(
      caller,
      "stake_resources",
      [resourceType, amount, duration],
      options,
    );
  }

  /**
   * Claim accumulated yield
   */
  async claimYield(
    caller: Keypair,
    stakeId: bigint,
    options?: TransactionOptions,
  ): Promise<TxResult<bigint>> {
    return this.executeTransaction(caller, "claim_yield", [stakeId], options);
  }

  /**
   * Execute a transaction on the contract
   */
  private async executeTransaction(
    caller: Keypair,
    method: string,
    args: any[],
    options?: TransactionOptions,
  ): Promise<TxResult> {
    try {
      const account = await this.server.getAccount(caller.publicKey());

      const operation = this.contract.call(method, ...args);

      const transaction = new TransactionBuilder(account, {
        fee: options?.fee || BASE_FEE,
        networkPassphrase: this.config.networkPassphrase,
      })
        .addOperation(operation)
        .setTimeout(options?.timeout || 30)
        .build();

      transaction.sign(caller);

      const response = await this.server.sendTransaction(transaction);

      if (response.status === "PENDING") {
        const txResult = await this.waitForTransaction(response.hash);
        return {
          success: true,
          result: txResult,
          txHash: response.hash,
        };
      }

      return {
        success: false,
        error: "Transaction failed",
      };
    } catch (error: any) {
      return {
        success: false,
        error: error.message || "Unknown error",
      };
    }
  }

  /**
   * Wait for transaction confirmation
   */
  private async waitForTransaction(
    hash: string,
    timeout = 30000,
  ): Promise<any> {
    const startTime = Date.now();

    while (Date.now() - startTime < timeout) {
      try {
        const tx = await this.server.getTransaction(hash);
        if (tx.status !== "NOT_FOUND") {
          return tx;
        }
      } catch (error) {
        // Continue polling
      }
      await new Promise((resolve) => setTimeout(resolve, 1000));
    }

    throw new Error("Transaction timeout");
  }
}
