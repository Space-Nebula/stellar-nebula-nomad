/**
 * Example: validated nebula layouts with expiry (module: nebula_gen).
 *
 *   NEBULA_GEN_CONTRACT_ID=C... npx tsx nebula-generator.ts
 *
 * Uses the standalone NebulaGen contract: validated inputs, per-ship
 * layouts that expire after the configured TTL, and anomaly queries.
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

interface Anomaly {
  x: bigint;
  y: bigint;
  rarity: bigint;
  anomaly_type: [string];
  resource_class: [string];
}
interface NebulaGenLayout {
  ship_id: bigint;
  region_id: bigint;
  layout_hash: Buffer;
  anomalies: Anomaly[];
  size: number;
  generated_at: bigint;
}

const LAYOUT_NOT_FOUND = 5;

async function main() {
  const genId = requireContractId(config.nebulaGenContractId, "NEBULA_GEN_CONTRACT_ID");
  const pilot = await loadPlayer();
  const shipId = 42n;
  const regionId = 7n;

  // Read the active layout, regenerating it if it has expired.
  async function activeLayout(): Promise<NebulaGenLayout> {
    const existing = await view<NebulaGenLayout | undefined>(
      pilot,
      "get_layout",
      [arg.u64(shipId)],
      genId,
    );
    if (existing) return existing;

    console.log("No live layout: generating a new one…");
    return invoke<NebulaGenLayout>(
      pilot,
      "generate_validated_nebula_layout",
      [arg.address(pilot.publicKey()), arg.u64(shipId), arg.u64(regionId), arg.bytes32(randomSeed())],
      genId,
    );
  }

  const layout = await activeLayout();
  console.log(`Layout ${layout.layout_hash.toString("hex").slice(0, 16)}… with ${layout.size} anomalies`);

  // Query a single anomaly (cheap, read-only).
  try {
    const a = await view<Anomaly>(pilot, "query_anomaly", [arg.u64(shipId), arg.u32(0)], genId);
    console.log(`Anomaly #0 at (${a.x}, ${a.y}): ${a.anomaly_type[0]} / ${a.resource_class[0]}`);
  } catch (e) {
    if (e instanceof ContractCallError && e.code === LAYOUT_NOT_FOUND) {
      console.log("Layout expired between calls; call activeLayout() again.");
    } else {
      throw e;
    }
  }
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
