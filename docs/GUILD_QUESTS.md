# Guild Quests and Cooperation (Issue #595)

Guild quests are cooperative objectives shared by an alliance. Members work on
the same progress counter, rewards are split between the guild treasury and the
contributors, and quests can rotate on daily/weekly/seasonal schedules or form
ordered chains.

## Data model

All records live in persistent contract storage under `GuildQuestKey` /`GuildEconomyKey`.
The original `GuildQuest` record is unchanged so existing indexers keep working:

| Field | Meaning |
| --- | --- |
| `quest_id`, `alliance_id` | Auto-incrementing quest ID scoped to the alliance |
| `quest_type` | `Symbol` matched by gameplay calls (`scan`, `harvest`, `boss_kill`, …) |
| `target_count`, `current_progress` | Guild-wide goal and accumulated progress |
| `reward_essence`, `reward_xp` | Base payout before tier scaling |
| `expires_at`, `completed` | Lifecycle fields |

New companion records:

- `GuildQuestMeta` — frequency, reward tier, `min_contributors`, chain position
  (`chain_id`, `chain_index`, `previous_quest_id`), treasury share in bps,
  rotation period and optional source template. Legacy quests have no meta and
  behave exactly as before.
- `GuildQuestChain` — ordered `quest_ids` plus `completed_count`; the next quest
  only accepts progress once the previous step is complete.
- `QuestAssignment` — optional roster of eligible members. No assignment means
  the quest is open to every alliance member.
- `Contribution(alliance, quest, player) -> u64` and
  `Contributors(alliance, quest) -> Vec<Address>` — per-member totals and the
  distinct contributor list used for the cooperation gate.
- `Rotation(alliance, template, period) -> quest_id` — makes spawning a
  recurring quest idempotent for one period.
- `QuestRewardPayout` / `QuestClaim` in `guild_economy` — immutable payout record
  and per-member claim flags.

### Progress and cooperation

`contribute_quest_progress` derives the caller's alliance, then for every
matching active quest:

1. Skips expired, completed, chain-locked, out-of-rotation or non-assigned
   quests.
2. Records the player's contribution and adds them to `Contributors` once.
3. Adds `amount` to the shared `current_progress`.
4. Completes the quest when `current_progress >= target_count` **and** the number
   of distinct contributors is at least `min_contributors`.

The default for legacy quests is `min_contributors = 1`. Callers that already
authenticate the player (for example `alliance_manager::attack_guild_boss`) can
call the module function directly; the contract entry point calls
`require_auth` before delegating.

## Reward tiers and distribution

Tiers multiply both essence and XP with integer basis-point math:

| Tier | Multiplier |
| --- | --- |
| 1 — standard | 10,000 bps (1×) |
| 2 — uncommon | 12,500 bps (1.25×) |
| 3 — rare | 15,000 bps (1.5×) |
| 4 — legendary | 20,000 bps (2×) |

Out-of-range tiers clamp to the nearest valid tier. `calculate_reward_split`
computes:

```text
scaled_essence      = base_essence * tier_bps / 10_000
treasury_amount     = scaled_essence * treasury_share_bps / 10_000
member_pool         = scaled_essence - treasury_amount
per_member          = member_pool / contributor_count
remainder_to_first  = member_pool % contributor_count
```

The remainder is credited to the first claimant, so the sum of all payouts is
exactly `scaled_essence`. `guild_economy::record_quest_rewards` credits the
treasury portion immediately, adds guild XP and stores a `QuestRewardPayout`.
Contributors call `claim_guild_quest_reward_share` once each; the claim requires
an initialized player profile and alliance membership. Treasury essence is
spendable through the existing `create_treasury_proposal` / `vote_on_proposal` /
`execute_proposal` flow.

Legacy `create_guild_quest` quests use a 10,000 bps treasury share, so their
entire reward still goes to the guild treasury and XP to the guild level.
Configured quests default to 6,000 bps (60% treasury / 40% contributors).

## Frequency rotation

`QuestFrequency` is one of `OneOff`, `Daily`, `Weekly` or `Seasonal`.
Rotation buckets use ledger time:

| Frequency | Bucket | Window |
| --- | --- | --- |
| Daily | `timestamp / 86_400` | 1 day |
| Weekly | `timestamp / 604_800` | 1 week |
| Seasonal | `timestamp / 7_776_000` | 90 days |

Configured recurring quests expire at the start of the next window, not at an
arbitrary duration. `rotate_guild_quests(admin, alliance, templates)` spawns the
current period's copies of template quests; it is idempotent for each
`(template, period)` pair. Queries (`get_active_guild_quests`,
`get_guild_quest_board`) hide recurring quests whose bucket is not the current
one, while leaving old records in storage for historical lookups.
`oneOff` quests always use period 0.

## Quest chains

1. `create_guild_quest_chain` defines an empty chain.
2. `create_guild_quest_with_config` is called with `chain_id` set; the quest is
   appended in creation order and records its predecessor.
3. A quest with a predecessor is skipped by `contribute_quest_progress` until
   `completed_count` reaches its `chain_index` (i.e. the previous step is done).
4. Completing a chained quest increments `completed_count`, unlocking the next
   step. Chains are limited to 12 quests.

Chains are intended for one-off quests; rotating templates with a chain ID are
rejected to avoid repeatedly appending to the same chain.

## API

Module functions are re-exported from the crate root and wrapped on
`NebulaNomadContract`:

| Entry point | Purpose |
| --- | --- |
| `create_guild_quest` | Original one-off quest, full treasury payout |
| `create_guild_quest_with_config` | Quest with frequency/tier/chain/cooperation settings |
| `create_guild_quest_chain` | Define an ordered chain |
| `assign_guild_quest` / `clear_guild_quest_assignment` | Manage the optional player roster |
| `contribute_guild_quest_progress` | Record progress (authenticates the player) |
| `rotate_guild_quests` | Spawn current-period recurring quests |
| `get_active_guild_quests` | Rotation-aware active quest list |
| `get_guild_quest_board` | Status-rich list for the UI |
| `get_guild_quest`, `get_guild_quest_chain`, `get_guild_quest_assignment`, `get_guild_quest_contributors` | Read-only queries |
| `get_guild_quest_payout` | Read the guild economy payout record |
| `claim_guild_quest_reward_share` | Claim a contributor's essence share |

Pure helpers (`tier_reward_bps`, `apply_reward_bps`, `calculate_reward_split`,
`rotation_period`, `rotation_expires_at`, `is_chain_unlocked`) are exported for
off-chain simulation and covered by small unit tests in `src/guild_quests.rs`.

Errors are `GuildQuestError` (1–9) and, for payouts, `GuildEconomyError` (9–12).
They implement/participate in the same error conventions as other modules;
`GuildQuestError` implements `StandardContractError`.

### Events

| Topics | Payload |
| --- | --- |
| `("g_quest", "created")` | `(alliance_id, quest_id, quest_type, target_count)` |
| `("g_quest", "config")` | `(alliance_id, quest_id, frequency, tier, min_contributors)` |
| `("g_quest", "progress")` | `(alliance_id, quest_id, player, amount, contributor_count)` |
| `("g_quest", "done")` | `(alliance_id, quest_id, base_essence, base_xp)` |
| `("g_quest", "tierdone")` | `(alliance_id, quest_id, tier_bps, scaled_essence)` |
| `("g_quest", "chainnew")` / `("g_quest", "rotated")` | Chain / rotation creation details |
| `("guild_ec", "questpay")` / `("guild_ec", "qclaim")` | Treasury funding / member claim |

## Frontend

`frontend/src/components/GuildQuestBoard.tsx` renders `GuildQuestSummary` rows
with status, frequency and tier badges, guild-wide progress bars, contributor
counts, expiry time and optional contribute/claim callbacks. It uses only React
state and hooks and is exported from `frontend/src/components/index.ts`.
