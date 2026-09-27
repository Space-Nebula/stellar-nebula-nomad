/**
 * Example: follow game activity through contract events.
 *
 *   NEBULA_CONTRACT_ID=C... npx tsx events.ts
 *
 * Contracts publish events with symbol topics, e.g.
 *   ("nebula", "scanned") -> (player, layout_hash, rarity)
 *   ("ship",   "minted")  -> (id, owner, ship_type, hull, scanner_power, …)
 *   ("pvp",    "end")     -> (combat_id, winner, duration)
 *
 * This script polls `getEvents` and prints them. Use it as a starting point
 * for leaderboards, activity feeds or notifications. For production, run
 * the subgraph/indexer under `subgraph/`.
 */
import { scValToNative, xdr } from "@stellar/stellar-sdk";
import { config, requireContractId, server } from "./lib/contract";

/** Encode a symbol topic the way `getEvents` filters expect (base64 XDR). */
const topic = (s: string) => xdr.ScVal.scvSymbol(s).toXDR("base64");

const FILTERS = [
  [topic("nebula"), topic("scanned")],
  [topic("ship"), topic("minted")],
  [topic("pvp"), "*"],
];

async function main() {
  const contractId = requireContractId(config.contractId, "NEBULA_CONTRACT_ID");

  // Start ~1 hour back (720 ledgers at ~5 s each).
  const latest = await server.getLatestLedger();
  let cursor: string | undefined;
  let startLedger: number | undefined = Math.max(1, latest.sequence - 720);

  for (let poll = 0; poll < 10; poll++) {
    const page = await server.getEvents({
      ...(cursor ? { cursor } : { startLedger: startLedger! }),
      filters: FILTERS.map((topics) => ({
        type: "contract" as const,
        contractIds: [contractId],
        topics: [topics],
      })),
      limit: 100,
    });

    for (const ev of page.events) {
      const topics = ev.topic.map((t) => String(scValToNative(t)));
      const value = scValToNative(ev.value);
      console.log(`[ledger ${ev.ledger}] ${topics.join(".")}`, value);
      cursor = ev.pagingToken;
    }

    startLedger = undefined;
    await new Promise((r) => setTimeout(r, 5_000));
  }
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
