/**
 * Example: mint, customise and trade ship NFTs (module: ship_nft).
 *
 *   NEBULA_CONTRACT_ID=C... npx tsx ship-nft.ts
 */
import { Keypair } from "@stellar/stellar-sdk";
import {
  ContractCallError,
  arg,
  config,
  fundWithFriendbot,
  invoke,
  loadPlayer,
  requireContractId,
  view,
} from "./lib/contract";

interface ShipNft {
  id: bigint;
  owner: string;
  ship_type: string;
  hull: number;
  scanner_power: number;
  durability: number;
  max_durability: number;
  metadata: Buffer;
  metadata_uri: Buffer;
}

// ShipError codes (docs/ERROR_CODES.md → ship_nft).
const ShipError: Record<number, string> = {
  1: "ShipAlreadyExists",
  2: "ShipNotFound",
  3: "NotOwner",
  4: "SameOwner",
  5: "BatchLimitExceeded",
  6: "InvalidShipType",
  7: "ReentrancyDetected",
  8: "InvalidMetadataUri",
};

async function main() {
  requireContractId(config.contractId, "NEBULA_CONTRACT_ID");
  const owner = await loadPlayer();

  // Step 1: mint. ship_type is a Symbol: "fighter" | "explorer" | "hauler".
  const ship = await invoke<ShipNft>(owner, "mint_ship", [
    arg.address(owner.publicKey()),
    arg.symbol("explorer"),
    arg.bytes(new Uint8Array()), // on-chain metadata bytes (keep small)
  ]);
  console.log(`Minted ship #${ship.id}: hull=${ship.hull} scanner=${ship.scanner_power}`);

  // Step 2: batch mint (max 3 per tx).
  const fleet = await invoke<ShipNft[]>(owner, "batch_mint_ships", [
    arg.address(owner.publicKey()),
    arg.vec([arg.symbol("fighter"), arg.symbol("hauler")]),
    arg.bytes(new Uint8Array()),
  ]);
  console.log(`Batch minted ${fleet.length} ships`);

  // Step 3: marketplace metadata (ipfs://, https:// or ar://).
  await invoke(owner, "set_metadata", [
    arg.address(owner.publicKey()),
    arg.u64(ship.id),
    arg.bytes("ipfs://bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi"),
  ]);

  // Step 4: list the owner's ships (read-only).
  const ids = await view<bigint[]>(owner, "get_ships_by_owner", [arg.address(owner.publicKey())]);
  console.log(`Owner has ships: ${ids.join(", ")}`);

  // Step 5: transfer to a friend.
  const friend = Keypair.random();
  await fundWithFriendbot(friend.publicKey());
  const moved = await invoke<ShipNft>(owner, "transfer_ship", [
    arg.u64(ship.id),
    arg.address(owner.publicKey()),
    arg.address(friend.publicKey()),
  ]);
  console.log(`Ship #${moved.id} now owned by ${moved.owner}`);

  // Step 6: expected failure. The original owner can't move it again.
  try {
    await invoke(owner, "transfer_ship", [
      arg.u64(ship.id),
      arg.address(owner.publicKey()),
      arg.address(friend.publicKey()),
    ]);
  } catch (e) {
    if (e instanceof ContractCallError && e.code !== null) {
      console.log(`Rejected as expected: ${ShipError[e.code] ?? `#${e.code}`}`);
    } else {
      throw e;
    }
  }
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
