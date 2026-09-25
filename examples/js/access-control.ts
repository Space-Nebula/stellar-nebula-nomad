/**
 * Example: admin tooling for roles and permissions (module: access_control).
 *
 *   NEBULA_CONTRACT_ID=C... PLAYER_SECRET=S...(admin) npx tsx access-control.ts <grantee G...>
 *
 * Run as the contract admin. Grants the `indexer` role to an address, checks
 * it, and shows the errors a non-admin gets.
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

const ADMIN_REQUIRED = 1; // AccessControlError::AdminRequired

async function main() {
  requireContractId(config.contractId, "NEBULA_CONTRACT_ID");
  const admin = await loadPlayer();
  const grantee = process.argv[2] ?? Keypair.random().publicKey();

  // Bootstrap once after deployment (fails harmlessly if already done).
  await invoke(admin, "init_rbac", [arg.address(admin.publicKey())]).catch((e) =>
    console.log(`init_rbac skipped: ${(e as Error).message}`),
  );

  // Grant with no expiry (Option<u32>::None -> scvVoid).
  await invoke(admin, "grant_role", [
    arg.address(admin.publicKey()),
    arg.symbol("indexer"),
    arg.address(grantee),
    arg.none(),
  ]);
  const has = await view<boolean>(admin, "has_role", [arg.symbol("indexer"), arg.address(grantee)]);
  console.log(`${grantee} has indexer role: ${has}`);

  // A non-admin trying the same thing is rejected.
  const outsider = Keypair.random();
  await fundWithFriendbot(outsider.publicKey());
  try {
    await invoke(outsider, "grant_role", [
      arg.address(outsider.publicKey()),
      arg.symbol("indexer"),
      arg.address(outsider.publicKey()),
      arg.none(),
    ]);
  } catch (e) {
    if (e instanceof ContractCallError && e.code === ADMIN_REQUIRED) {
      console.log("Non-admin grant rejected (AdminRequired)");
    } else {
      throw e;
    }
  }
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
