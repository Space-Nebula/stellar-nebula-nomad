/**
 * Example: scan a nebula from a dApp (module: nebula_explorer).
 *
 *   NEBULA_CONTRACT_ID=C... npx tsx nebula-exploration.ts
 *
 * Steps:
 *   1. preview a layout with a free simulation (`generate_nebula_layout`),
 *   2. submit `scan_nebula`, which generates the layout, scores its rarity
 *      (same logic as `calculate_rarity_tier`) and emits the event indexers
 *      and the frontend listen for.
 */
import { arg, config, invoke, loadPlayer, randomSeed, requireContractId, view } from "./lib/contract";

interface NebulaCell {
  x: number;
  y: number;
  cell_type: unknown;
  energy: number;
}
interface NebulaLayout {
  width: number;
  height: number;
  cells: NebulaCell[];
  seed: Buffer;
  timestamp: bigint;
  total_energy: number;
}

async function main() {
  requireContractId(config.contractId, "NEBULA_CONTRACT_ID");
  const player = await loadPlayer();

  // A fresh 32-byte seed. The contract mixes in ledger data, so the result
  // depends on the ledger the transaction lands in.
  const seed = randomSeed();

  // Step 1: free preview via simulation (no fee, no state change).
  const preview = await view<NebulaLayout>(player, "generate_nebula_layout", [
    arg.bytes32(seed),
    arg.address(player.publicKey()),
  ]);
  console.log(`Preview: ${preview.width}x${preview.height}, energy ${preview.total_energy}`);

  // Step 2: submit the scan. Returns (NebulaLayout, Rarity) as a 2-tuple.
  const [layout, rarity] = await invoke<[NebulaLayout, unknown]>(player, "scan_nebula", [
    arg.bytes32(seed),
    arg.address(player.publicKey()),
  ]);
  const rarityName = Array.isArray(rarity) ? rarity[0] : rarity; // enums decode as ["Variant"]
  console.log(`Scanned: energy ${layout.total_energy}, rarity ${String(rarityName)}`);

  // Show the richest cells so the UI can highlight them.
  const top = [...layout.cells].sort((a, b) => b.energy - a.energy).slice(0, 5);
  for (const c of top) {
    console.log(`  (${c.x},${c.y}) energy=${c.energy}`);
  }
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
