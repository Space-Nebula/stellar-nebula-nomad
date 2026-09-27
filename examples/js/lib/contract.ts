/**
 * Shared helpers for the TypeScript examples.
 *
 * Every example follows the same Soroban flow:
 *
 *   read-only call:  build tx -> simulate -> decode `retval`
 *   state change:    build tx -> simulate -> assemble (adds footprint + fees)
 *                    -> sign -> send -> poll until SUCCESS / FAILED
 *
 * Configuration comes from environment variables so the same scripts run
 * against testnet, futurenet or a local `stellar quickstart` node:
 *
 *   NEBULA_CONTRACT_ID      main NebulaNomadContract id (C...)
 *   NEBULA_GEN_CONTRACT_ID  NebulaGen contract id (optional)
 *   STELLAR_RPC_URL         default: https://soroban-testnet.stellar.org
 *   STELLAR_NETWORK         "testnet" | "futurenet" | "local" (default testnet)
 *   PLAYER_SECRET           S... secret key (a funded account)
 */
import {
  Address,
  BASE_FEE,
  Contract,
  Keypair,
  Networks,
  SorobanRpc,
  TransactionBuilder,
  nativeToScVal,
  scValToNative,
  xdr,
} from "@stellar/stellar-sdk";

// ─── Configuration ─────────────────────────────────────────────────────────

const PASSPHRASES: Record<string, string> = {
  testnet: Networks.TESTNET,
  futurenet: Networks.FUTURENET,
  local: Networks.STANDALONE,
};

export const config = {
  rpcUrl: process.env.STELLAR_RPC_URL ?? "https://soroban-testnet.stellar.org",
  networkPassphrase: PASSPHRASES[process.env.STELLAR_NETWORK ?? "testnet"] ?? Networks.TESTNET,
  contractId: process.env.NEBULA_CONTRACT_ID ?? "",
  nebulaGenContractId: process.env.NEBULA_GEN_CONTRACT_ID ?? "",
};

export const server = new SorobanRpc.Server(config.rpcUrl, {
  allowHttp: config.rpcUrl.startsWith("http://"),
});

/** Load the player keypair, or create and fund a throwaway one via Friendbot. */
export async function loadPlayer(): Promise<Keypair> {
  if (process.env.PLAYER_SECRET) {
    return Keypair.fromSecret(process.env.PLAYER_SECRET);
  }
  const kp = Keypair.random();
  await fundWithFriendbot(kp.publicKey());
  console.log(`Created throwaway account ${kp.publicKey()}`);
  return kp;
}

/** Fund an account on testnet/futurenet (no-op on networks without Friendbot). */
export async function fundWithFriendbot(publicKey: string): Promise<void> {
  const network = process.env.STELLAR_NETWORK ?? "testnet";
  const url =
    network === "futurenet"
      ? "https://friendbot-futurenet.stellar.org"
      : "https://friendbot.stellar.org";
  if (network === "local") return;
  const res = await fetch(`${url}?addr=${encodeURIComponent(publicKey)}`);
  if (!res.ok && res.status !== 400 /* already funded */) {
    throw new Error(`Friendbot failed: ${res.status} ${await res.text()}`);
  }
}

export function requireContractId(id: string, envVar: string): string {
  if (!id) {
    throw new Error(`Set ${envVar} to the deployed contract id (C...).`);
  }
  return id;
}

// ─── Argument builders ─────────────────────────────────────────────────────
// Contract.call() takes xdr.ScVal values. These helpers make the Rust types
// in each contract signature explicit at the call site.

export const arg = {
  address: (a: string) => new Address(a).toScVal(),
  u32: (n: number) => nativeToScVal(n, { type: "u32" }),
  u64: (n: number | bigint) => nativeToScVal(BigInt(n), { type: "u64" }),
  i128: (n: number | bigint) => nativeToScVal(BigInt(n), { type: "i128" }),
  bool: (b: boolean) => nativeToScVal(b),
  symbol: (s: string) => nativeToScVal(s, { type: "symbol" }),
  bytes: (b: Uint8Array | string) =>
    nativeToScVal(typeof b === "string" ? Buffer.from(b, "utf8") : Buffer.from(b)),
  /** BytesN<32>: must be exactly 32 bytes. */
  bytes32: (b: Uint8Array) => {
    if (b.length !== 32) throw new Error(`expected 32 bytes, got ${b.length}`);
    return nativeToScVal(Buffer.from(b));
  },
  none: () => xdr.ScVal.scvVoid(),
  vec: (items: xdr.ScVal[]) => xdr.ScVal.scvVec(items),
};

/** 32 cryptographically random bytes, suitable as a nebula seed. */
export function randomSeed(): Uint8Array {
  const seed = new Uint8Array(32);
  crypto.getRandomValues(seed);
  return seed;
}

// ─── Errors ────────────────────────────────────────────────────────────────

/** Thrown when a simulation or transaction fails. `code` is the contract error number. */
export class ContractCallError extends Error {
  constructor(
    public readonly method: string,
    public readonly detail: string,
    public readonly code: number | null = contractErrorCode(detail),
  ) {
    super(`${method} failed${code !== null ? ` with contract error #${code}` : ""}: ${detail}`);
  }
}

/** Extract N from a host error string containing `Error(Contract, #N)`. */
export function contractErrorCode(text: string): number | null {
  const m = /Error\(Contract, #(\d+)\)/.exec(text);
  return m ? Number(m[1]) : null;
}

// ─── Calls ─────────────────────────────────────────────────────────────────

async function buildTx(sourcePublicKey: string, contractId: string, method: string, args: xdr.ScVal[]) {
  const account = await server.getAccount(sourcePublicKey);
  return new TransactionBuilder(account, {
    fee: BASE_FEE,
    networkPassphrase: config.networkPassphrase,
  })
    .addOperation(new Contract(contractId).call(method, ...args))
    .setTimeout(30)
    .build();
}

/**
 * Read-only call. Nothing is submitted, so it costs no fee.
 * `source` can be any existing account; it is only used to build the tx.
 */
export async function view<T = unknown>(
  source: Keypair,
  method: string,
  args: xdr.ScVal[] = [],
  contractId = config.contractId,
): Promise<T> {
  const tx = await buildTx(source.publicKey(), contractId, method, args);
  const sim = await server.simulateTransaction(tx);
  if (SorobanRpc.Api.isSimulationError(sim)) {
    throw new ContractCallError(method, sim.error);
  }
  if (!SorobanRpc.Api.isSimulationSuccess(sim) || !sim.result) {
    throw new ContractCallError(method, "simulation returned no result");
  }
  return scValToNative(sim.result.retval) as T;
}

/**
 * State-changing call: simulate, assemble, sign, submit and wait.
 * Returns the decoded return value of the contract function.
 */
export async function invoke<T = unknown>(
  signer: Keypair,
  method: string,
  args: xdr.ScVal[] = [],
  contractId = config.contractId,
): Promise<T> {
  const tx = await buildTx(signer.publicKey(), contractId, method, args);

  // 1. Simulate: surfaces contract errors before paying any fee, and
  //    computes the storage footprint + resource fee.
  const sim = await server.simulateTransaction(tx);
  if (SorobanRpc.Api.isSimulationError(sim)) {
    throw new ContractCallError(method, sim.error);
  }

  // 2. Assemble: attach footprint, auth entries and resource fee.
  const prepared = SorobanRpc.assembleTransaction(tx, sim).build();
  prepared.sign(signer);

  // 3. Submit.
  const sent = await server.sendTransaction(prepared);
  if (sent.status === "ERROR") {
    throw new ContractCallError(method, JSON.stringify(sent.errorResult ?? sent));
  }

  // 4. Poll until the ledger closes.
  for (let attempt = 0; attempt < 30; attempt++) {
    const res = await server.getTransaction(sent.hash);
    if (res.status === SorobanRpc.Api.GetTransactionStatus.SUCCESS) {
      return (res.returnValue ? scValToNative(res.returnValue) : undefined) as T;
    }
    if (res.status === SorobanRpc.Api.GetTransactionStatus.FAILED) {
      throw new ContractCallError(method, `transaction ${sent.hash} failed`);
    }
    await new Promise((r) => setTimeout(r, 1_000));
  }
  throw new ContractCallError(method, `timed out waiting for ${sent.hash}`);
}
