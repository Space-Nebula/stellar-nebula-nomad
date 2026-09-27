/**
 * Example: first-session onboarding flow
 * (modules: player_profile, onboarding_tutorial, session_manager).
 *
 *   NEBULA_CONTRACT_ID=C... npx tsx player-onboarding.ts
 *
 * The typical first-launch sequence for a new wallet:
 *   profile -> tutorial steps -> first ship -> play session -> record progress
 */
import {
  ContractCallError,
  arg,
  config,
  invoke,
  loadPlayer,
  requireContractId,
  view,
} from "./lib/contract";

const TOTAL_STEPS = 5;
const PROFILE_ALREADY_EXISTS = 2; // ProfileError::ProfileAlreadyExists

interface PlayerProfile {
  id: bigint;
  owner: string;
  total_scans: number;
  essence_earned: bigint;
}
interface TutorialProgress {
  next_step: number;
  completed_mask: number;
  completed_count: number;
}

async function main() {
  requireContractId(config.contractId, "NEBULA_CONTRACT_ID");
  const player = await loadPlayer();
  const me = arg.address(player.publicKey());

  // Step 1: create the profile (idempotent from the UI's point of view).
  let profileId: bigint;
  try {
    profileId = await invoke<bigint>(player, "initialize_profile", [me]);
    console.log(`Profile #${profileId} created`);
  } catch (e) {
    if (e instanceof ContractCallError && e.code === PROFILE_ALREADY_EXISTS) {
      // Conflict errors are "already done". Look the profile up instead.
      // The id isn't returned again, so persist it on first creation
      // (or recover it from the indexer / profile-created events).
      if (!process.env.PROFILE_ID) {
        throw new Error("Profile already exists: set PROFILE_ID to its id to continue.");
      }
      profileId = BigInt(process.env.PROFILE_ID);
      console.log(`Profile #${profileId} already exists; continuing`);
    } else {
      throw e;
    }
  }

  // Step 2: tutorial. Each completed step returns its starter reward.
  await invoke(player, "create_profile_onboarding", [me]).catch(() => undefined);
  await invoke(player, "start_tutorial", [me]).catch(() => undefined);
  for (let step = 0; step < TOTAL_STEPS; step++) {
    try {
      const reward = await invoke<bigint>(player, "complete_tutorial_step", [me, arg.u32(step)]);
      console.log(`  step ${step}: +${reward}`);
    } catch (e) {
      console.log(`  step ${step}: ${(e as Error).message}`);
    }
  }
  const progress = await view<TutorialProgress | undefined>(player, "get_tutorial_progress", [me]);
  console.log(`Tutorial: ${progress?.completed_count ?? 0}/${TOTAL_STEPS} steps`);

  // Step 3: first ship + session.
  const ship = await invoke<{ id: bigint }>(player, "mint_ship", [
    me,
    arg.symbol("explorer"),
    arg.bytes(new Uint8Array()),
  ]);
  const sessionId = await invoke<bigint>(player, "start_session", [me, arg.u64(ship.id)]);
  console.log(`Session #${sessionId} started with ship #${ship.id}`);

  // Step 4: after gameplay, record progress (3 scans, 150 essence).
  await invoke(player, "update_progress", [me, arg.u64(profileId), arg.u32(3), arg.i128(150)]);
  const profile = await view<PlayerProfile>(player, "get_profile", [arg.u64(profileId)]);
  console.log(`Profile: scans=${profile.total_scans} essence=${profile.essence_earned}`);
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
