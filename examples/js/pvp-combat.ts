/**
 * Example: PvP challenge and turn-based combat (module: pvp_combat).
 *
 *   NEBULA_CONTRACT_ID=C... npx tsx pvp-combat.ts
 *
 * Two local accounts challenge each other and alternate moves until the
 * combat ends. Both must sign their own moves.
 */
import { Keypair } from "@stellar/stellar-sdk";
import { arg, config, fundWithFriendbot, invoke, loadPlayer, requireContractId, view } from "./lib/contract";

interface CombatState {
  combat_id: bigint;
  player1: string;
  player2: string;
  player1_hp: number;
  player2_hp: number;
  player1_energy: number;
  player2_energy: number;
  turn: string;
  status: string;
  winner?: string;
}

async function main() {
  requireContractId(config.contractId, "NEBULA_CONTRACT_ID");
  const orion = await loadPlayer();
  const vega = Keypair.random();
  await fundWithFriendbot(vega.publicKey());
  const players: Record<string, Keypair> = {
    [orion.publicKey()]: orion,
    [vega.publicKey()]: vega,
  };

  // Step 1: challenge (stake 0 = friendly match).
  const challengeId = await invoke<bigint>(orion, "create_challenge", [
    arg.address(orion.publicKey()),
    arg.address(vega.publicKey()),
    arg.i128(0),
  ]);
  console.log(`Challenge #${challengeId} sent`);

  // Step 2: the opponent accepts, which starts the combat.
  const combatId = await invoke<bigint>(vega, "accept_challenge", [
    arg.address(vega.publicKey()),
    arg.u64(challengeId),
  ]);
  console.log(`Combat #${combatId} started`);

  // Step 3: alternate moves. Read state (free), then the player on turn signs.
  for (let round = 1; round <= 40; round++) {
    const s = await view<CombatState>(orion, "get_combat", [arg.u64(combatId)]);
    if (s.status !== "active") break;

    const actor = players[s.turn];
    const energy = s.turn === s.player1 ? s.player1_energy : s.player2_energy;
    // Simple strategy: attack when there's enough energy, otherwise defend.
    const move = energy >= 15 ? "attack" : "defend";
    const power = move === "attack" ? 30 : 0;

    await invoke(actor, "execute_combat_move", [
      arg.address(actor.publicKey()),
      arg.u64(combatId),
      arg.symbol(move),
      arg.u32(power),
    ]);
    console.log(`  round ${round}: ${move}(${power})`);
  }

  // Step 4: results and ratings.
  const final = await view<CombatState>(orion, "get_combat", [arg.u64(combatId)]);
  console.log(`Status: ${final.status}, winner: ${final.winner ?? "none"}`);
  for (const kp of [orion, vega]) {
    const elo = await view<number>(orion, "get_elo_rating", [arg.address(kp.publicKey())]);
    console.log(`  ${kp.publicKey().slice(0, 6)}… ELO ${elo}`);
  }
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
