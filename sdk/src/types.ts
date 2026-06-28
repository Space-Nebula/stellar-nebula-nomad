import { Account, Operation, Transaction, xdr } from "@stellar/stellar-sdk";

// Core types
export interface ContractConfig {
  contractId: string;
  networkPassphrase: string;
  rpcUrl: string;
}

export interface TransactionOptions {
  fee?: string;
  timeout?: number;
}

// Ship types
export interface Ship {
  id: bigint;
  owner: string;
  shipType: ShipType;
  rarity: Rarity;
  stats: ShipStats;
}

export enum ShipType {
  Explorer = 0,
  Fighter = 1,
  Trader = 2,
  Miner = 3,
}

export enum Rarity {
  Common = 0,
  Uncommon = 1,
  Rare = 2,
  Epic = 3,
  Legendary = 4,
}

export interface ShipStats {
  speed: number;
  cargo: number;
  weapons: number;
  shields: number;
}

// Nebula types
export interface NebulaLayout {
  seed: bigint;
  width: number;
  height: number;
  cells: Cell[];
  rarity: Rarity;
  timestamp: bigint;
}

export interface Cell {
  x: number;
  y: number;
  cellType: CellType;
  energy: number;
}

export enum CellType {
  Empty = 0,
  Resource = 1,
  Hazard = 2,
  Portal = 3,
}

// Resource types
export interface ResourceBalance {
  resourceType: ResourceType;
  amount: bigint;
}

export enum ResourceType {
  Fuel = 0,
  Minerals = 1,
  Alloys = 2,
  Crystals = 3,
}

// Event types
export interface ContractEvent {
  type: string;
  data: Record<string, any>;
  ledger: number;
  txHash: string;
}

// Transaction result
export interface TxResult<T = any> {
  success: boolean;
  result?: T;
  error?: string;
  txHash?: string;
}
