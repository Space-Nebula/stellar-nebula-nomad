/**
 * Example: decode and react to contract errors in a dApp.
 * Companion to docs/ERROR_CODES.md.
 *
 *   NEBULA_CONTRACT_ID=C... NEBULA_GEN_CONTRACT_ID=C... npx tsx error-handling.ts
 *
 * Key points:
 *   * contract errors reach clients as `Error(Contract, #N)`,
 *   * N is only unique *per module*, so decode it with that module's table,
 *   * decide per category: fix input, run a prerequisite, back off, or report.
 */
import {
  ContractCallError,
  arg,
  config,
  invoke,
  loadPlayer,
  randomSeed,
  requireContractId,
  view,
} from "./lib/contract";

type Category = "validation" | "auth" | "not_found" | "conflict" | "limit" | "state" | "setup" | "internal";

interface ErrorInfo {
  name: string;
  category: Category;
  hint: string;
}

/** Per-module tables. Extend with the modules your dApp calls. */
const ERRORS: Record<string, Record<number, ErrorInfo>> = {
  nebula_gen: {
    1: { name: "NotInitialized", category: "setup", hint: "Contract not initialised by admin." },
    2: { name: "AlreadyInitialized", category: "conflict", hint: "Already initialised." },
    3: { name: "InvalidSeed", category: "validation", hint: "Seed must not be all zeros." },
    4: { name: "InvalidIndex", category: "validation", hint: "Anomaly index out of range." },
    5: { name: "LayoutNotFound", category: "not_found", hint: "Layout missing or expired: regenerate." },
    6: { name: "InvalidSize", category: "validation", hint: "Size outside [min,max]." },
    7: { name: "InvalidTtl", category: "validation", hint: "TTL must be > 0." },
    8: { name: "InvalidShipId", category: "validation", hint: "ship_id must be > 0." },
    9: { name: "InvalidRegionId", category: "validation", hint: "region_id must be 1..=1,000,000." },
    10: { name: "AnomalyOutOfBounds", category: "validation", hint: "Index >= layout size." },
  },
  ship_nft: {
    1: { name: "ShipAlreadyExists", category: "internal", hint: "Report: id collision." },
    2: { name: "ShipNotFound", category: "not_found", hint: "Check the ship id." },
    3: { name: "NotOwner", category: "auth", hint: "Sign with the ship owner." },
    4: { name: "SameOwner", category: "validation", hint: "Recipient is already the owner." },
    5: { name: "BatchLimitExceeded", category: "limit", hint: "Mint at most 3 ships per tx." },
    6: { name: "InvalidShipType", category: "validation", hint: "Use fighter / explorer / hauler." },
    7: { name: "ReentrancyDetected", category: "internal", hint: "Report: re-entrant call." },
    8: { name: "InvalidMetadataUri", category: "validation", hint: "Use ipfs://, https:// or ar://." },
  },
};

export function describe(module: string, err: unknown): ErrorInfo {
  if (err instanceof ContractCallError && err.code !== null) {
    return (
      ERRORS[module]?.[err.code] ?? {
        name: `${module}#${err.code}`,
        category: "internal",
        hint: "Unknown code: see docs/ERROR_CODES.md",
      }
    );
  }
  return { name: "HostError", category: "internal", hint: String((err as Error)?.message ?? err) };
}

/**
 * Retry with exponential backoff only for categories that can resolve on
 * their own (limits, time windows). Everything else fails fast.
 */
export async function withRetry<T>(module: string, fn: () => Promise<T>, attempts = 4): Promise<T> {
  for (let i = 0; ; i++) {
    try {
      return await fn();
    } catch (e) {
      const info = describe(module, e);
      const retryable = info.category === "limit" || info.category === "state";
      if (!retryable || i >= attempts - 1) throw e;
      const delay = 2 ** i * 1_000;
      console.log(`${info.name}: retrying in ${delay} ms`);
      await new Promise((r) => setTimeout(r, delay));
    }
  }
}

async function main() {
  const player = await loadPlayer();

  // 1. Validation error, caught in simulation (no fee paid).
  if (config.nebulaGenContractId) {
    const genId = config.nebulaGenContractId;
    try {
      await view(player, "generate_validated_nebula_layout", [
        arg.address(player.publicKey()),
        arg.u64(0), // invalid: ship_id must be > 0
        arg.u64(1),
        arg.bytes32(randomSeed()),
      ], genId);
    } catch (e) {
      const info = describe("nebula_gen", e);
      console.log(`[${info.category}] ${info.name}: ${info.hint}`);
    }

    // 2. Not-found error with an automatic prerequisite.
    const shipId = 9n;
    try {
      await view(player, "query_anomaly", [arg.u64(shipId), arg.u32(0)], genId);
    } catch (e) {
      const info = describe("nebula_gen", e);
      if (info.name === "LayoutNotFound") {
        console.log("Layout missing: generating, then retrying once");
        await invoke(player, "generate_validated_nebula_layout", [
          arg.address(player.publicKey()),
          arg.u64(shipId),
          arg.u64(1),
          arg.bytes32(randomSeed()),
        ], genId);
        const a = await view(player, "query_anomaly", [arg.u64(shipId), arg.u32(0)], genId);
        console.log("Recovered anomaly:", a);
      } else {
        throw e;
      }
    }
  }

  // 3. Same number, different module: an unknown ship returns #2, which is
  //    ShipNotFound in ship_nft but AlreadyInitialized in nebula_gen.
  //    NotFound is not retryable, so withRetry fails fast here.
  requireContractId(config.contractId, "NEBULA_CONTRACT_ID");
  try {
    await withRetry("ship_nft", () =>
      view(player, "get_ship", [arg.u64(999_999_999)]),
    );
  } catch (e) {
    const info = describe("ship_nft", e);
    console.log(`[${info.category}] ${info.name}: ${info.hint}`);
  }
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
