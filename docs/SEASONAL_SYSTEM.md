# Seasonal Content and Rewards System (Issue #594)

The seasonal system gives every player a shared 90-day (three-month) ladder with
rotating themes, exclusive content, a two-track season pass, and seasonal
challenges. It is implemented across four contract modules:

| Module | Responsibility |
| --- | --- |
| `src/seasons.rs` | Season lifecycle, 90-day windows, chapters, archives, atomic rollover |
| `src/seasonal_rewards.rs` | Pass tier math, rotating content, challenge wrappers, dashboard/transition views |
| `src/battle_pass.rs` | Free/premium XP tables, reward claims, cosmetic registry |
| `src/event_scheduler.rs` | Time-bounded challenges, progress and reward claims |

`src/leaderboards.rs` archives and clears seasonal boards, and `src/elo_rating.rs`
performs the soft rating reset. The contract entry points live on
`NebulaNomadContract` in `src/lib.rs`; the React view is
`frontend/src/components/SeasonalDashboard.tsx`.

## 1. Season cycle

- A season is exactly `SEASON_DURATION_SECS = 90 * 24 * 60 * 60` seconds.
- It is split into `CHAPTERS_PER_SEASON = 3` chapters of `CHAPTER_DURATION_SECS =
  30 * 24 * 60 * 60` seconds. The active chapter is derived from elapsed time
  (`get_current_chapter`) and can be persisted with `advance_chapter`.
- `season_window(season_id, genesis)` returns the half-open `[start, end)` window
  for any season; `season_id_at(genesis, timestamp)` maps a ledger timestamp back
  to its season. All math is saturating.
- Season 1 starts when an admin calls `init_season`. Every later season starts
  immediately when its predecessor's window closes (no dead time).

### Theme and exclusive-content rotation

Six themes rotate in a fixed order:

| Season IDs | Theme | Exclusive nebula | Event pool multiplier |
| --- | --- | --- | --- |
| 1, 7, 13, … | EmberNebula | EmberCloud | 1.10× |
| 2, 8, 14, … | VoidTide | VoidRift | 1.25× |
| 3, 9, 15, … | StellarApex | CrystalPeak | 1.15× |
| 4, 10, 16, … | CrimsonDrift | CrimsonStorm | 1.30× |
| 5, 11, 17, … | AzureVeil | AzureGlacier | 1.20× |
| 6, 12, 18, … | NovaBurst | NovaCrater | 1.50× |

`SeasonTheme::from_season_id`, `seasonal_nebula_type` and
`seasons::rotation_index` are pure helpers. `seasonal_event_reward_pool` applies
the theme multiplier and an extra `EXCLUSIVE_EVENT_BONUS_BPS` (2 500 bps) for
exclusive events. `seasonal_rewards::exclusive_content_for` bundles the theme
symbol, nebula, cosmetic ID, event multiplier, and featured pass track for the
UI. The cosmetic ID is unique per season; the palette repeats only every six
seasons.

## 2. Ranking and progress resets

The full transition (`transition_season` / `ensure_season_transition`) performs
these writes in a single transaction. The legacy `rollover_season` keeps its
original side effects (steps 1–3 and 6 only) so existing callers and gas budgets
are unchanged.

1. Participation rewards are computed:
   `scans × 10 + essence × 500 / 10_000 + 500 if all three chapters were active`.
2. Per-player `ParticipantStats` records are removed (seasonal progress reset),
   while lifetime `ProfileSeasonCount` is incremented.
3. The ending season is written to `SeasonArchive`.
4. The seasonal leaderboards (`scans`, `essence`, `pvp_rating`, `pvp_wins`,
   `seas_score`) are archived under the ending ID, cleared, and their season
   counters advanced. Daily/weekly/monthly/all-time boards are untouched.
5. The ELO season counter is aligned with the new season ID and every supplied
   player's rating is compressed 10% toward the floor (`apply_seasonal_reset`).
6. The next season is written to `CurrentSeason` with the next theme, a fresh
   90-day window, `current_chapter = 1` and a new `leaderboard_reset_at`.

Season-window and reward math uses saturating integer math. Queries remain
available after the window closes: `get_season_phase` returns `Expired` until a
rollover runs.

## 3. Season pass rewards

`battle_pass::init_default_season_rewards_inner` installs two tracks for each
season:

- **Free** — tiers 1–30, `xp_required = 100 + (tier-1) × 150`, escalating
  essence amounts. Available to everyone.
- **Premium** — tiers 1–50, `xp_required = 200 + (tier-1) × 100`, escalating
  essence plus cosmetics every five tiers and title badges at tiers 25/50.
  Requires `unlock_premium_pass`; premium rewards are cosmetic-only by design.

XP is awarded automatically by the season activity hooks:
`record_seasonal_participation` updates season stats and calls
`battle_pass::add_xp` for scans/essence, while
`record_seasonal_nebula_discovery` grants the nebula-discovery bonus. Challenge
claims also grant the challenge XP bonus (deduplicated per challenge ID).

Pure helpers in `seasonal_rewards` (`tier_xp_required`,
`highest_unlocked_tier`, `next_unlocked_tier`) mirror the on-chain tables and
are reused by `get_season_pass_summary` and the frontend. Claiming goes through
`claim_season_pass_reward`, which validates the tier and delegates to
`battle_pass::claim_reward_v2` (authenticated by the player; duplicate claims
and unmet XP thresholds are rejected).

`init_season_pass` is idempotent and also stores the season's
`ExclusiveContent`. `rotate_exclusive_content` lets an admin refresh that
bundle after a content hotfix.

## 4. Seasonal challenges

`create_seasonal_challenge` wraps `event_scheduler` and rejects windows that
would extend beyond the current 90-day season. Challenges carry a target metric
(`scans`, `essence`, `pvp_wins`, …), a target value, and separate free/premium
rewards.

- `record_seasonal_challenge_progress` adds progress, caps at the target, and
  ignores increments after expiry; the challenge must belong to the active
  season.
- `claim_seasonal_challenge_reward` pays the premium rate to premium pass
  holders and the free rate to everyone else. Claims are idempotent and have a
  seven-day grace window after the challenge ends; the wrapper also accepts a
  challenge from the season archived by the most recent transition so a
  rollover does not cut that grace period short. Premium status is always read
  from the challenge's own season.
- `seasonal_challenges_for_season` returns currently open challenges for a
  season; `get_seasonal_challenge_status` returns a player's progress,
  completion, claim state, reward, and remaining time.
- Challenge completion also grants battle-pass XP through
  `battle_pass::add_xp_for_challenge` (deduplicated by challenge ID).

## 5. Smooth transitions

Use the idempotent keeper entry point `ensure_season_transition` (or the
contract method of the same name). It authenticates the admin once, returns
`None` while the season is active, and otherwise runs the full transition:
archive + ranking reset + next season + pass/exclusive-content provisioning.
`transition_season` performs the same work without the time guard (useful for
manual recovery). `get_season_dashboard` returns a single read containing the
active season, phase, remaining time, chapter, exclusive content, optional
player pass summary, and next-season start.

Both transition entry points emit a `("season", "trans")` event in addition to
the existing `("season", "rolled")` and ELO `("elo", "season")` events. Leaderboard
archiving inside a rollover is intentionally silent so the legacy rollover still
emits exactly one event; calling `reset_seasonal_leaderboard` directly still
emits its `("lb", "seas_rst")` event. Anyone can read `get_last_transition` for
the audit record.

## 6. Contract API summary

| Method | Purpose |
| --- | --- |
| `init_season(admin, title)` | Start season 1 |
| `get_current_season()` / `get_season_phase()` / `get_season_time_remaining()` / `is_transition_due()` | Season queries |
| `get_last_transition()` | Latest transition audit record |
| `get_current_chapter()` / `advance_chapter(admin)` | Chapter tracking |
| `record_seasonal_participation(profile_id, scans, essence)` | Player progress + pass XP |
| `record_seasonal_nebula_discovery(profile_id)` | Exclusive nebula discovery + bonus XP |
| `rollover_season(admin, title, participant_ids)` | Legacy-compatible rollover |
| `transition_season(...)` / `ensure_season_transition(...)` | Full/idempotent transition |
| `get_season_dashboard(profile_id?)` | Frontend payload |
| `init_season_pass(admin, season_id)` / `rotate_exclusive_content(...)` | Pass/content setup |
| `get_exclusive_content(season_id)` | Rotating content query |
| `get_season_pass_summary(profile_id)` / `unlock_premium_pass(...)` | Pass state |
| `claim_season_pass_reward(player, profile_id, tier, pass_tier)` | Reward claim |
| `create_seasonal_challenge(admin, spec, duration_secs)` | Challenge creation |
| `record_seasonal_challenge_progress(...)` / `claim_seasonal_challenge_reward(...)` | Challenge tracking/claim |
| `get_seasonal_challenges(season_id)` / `get_seasonal_challenge_status(...)` | Challenge queries |

Error variants are preserved in `SeasonError`; new failures use
`SeasonalRewardsError`, which implements `StandardContractError` with module
`seasonal_rewards`.

## 7. Frontend

`SeasonalDashboard.tsx` renders the season header/theme, 90-day progress bar,
chapter, pass XP and tier progress (free/premium), the exclusive rotating
content card, and challenge progress with claim buttons. It exports pure
helpers (`themeForSeason`, `tierXpRequired`, `highestUnlockedTier`,
`formatCountdown`) and is re-exported from the component barrel. No new npm
dependencies are required.

## 8. Tests

Pure-logic unit tests cover 90-day/chapter arithmetic, season-at-timestamp
mapping, theme and rotation cycles, reward pool scaling, expiry boundaries,
pass tier thresholds, challenge reward pro-rating, and exclusive content IDs.
They live in `#[cfg(test)] mod tests` in `seasons.rs` and
`seasonal_rewards.rs`.
