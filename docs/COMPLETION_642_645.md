# Battle Pass, Skin Marketplace, Content Tools, and NFT Marketplace Completion

This change closes the remaining acceptance gaps in Issues #642–#645 while
reusing the modules that already existed.

## #642 — Battle Pass and Progression Rewards

- `challenge_system` now persists challenges in per-period indexes
  (`DailyChallenges(day)`, `WeeklyChallenges(week)`, `SeasonalChallenges(season)`)
  and a global index, so daily and weekly boards roll over without deleting
  history.
- Challenge creation uses a monotonic ID, rejects zero targets/durations, and
  records the active window (`starts_at`/`ends_at`).
- Progress can only be written while a challenge is active; completion is
  idempotent.
- `claim_reward_for` authorizes the caller, rejects inactive/expired challenges,
  and marks the reward claimed exactly once.
- `battle_pass::claim_challenge_reward` claims the challenge and credits its
  configured XP to the active season's pass, with deduplication by challenge ID.
- Season initialization and rollover now seed both the 30-tier free track and
  the 50-tier premium track automatically.

New entry points include `create_bp_challenge`,
`update_bp_challenge_progress`, `claim_battle_pass_challenge`,
`get_active_bp_challenges`, and `get_current_bp_challenges`.

## #643 — Skin Marketplace and Trading

`skin_marketplace` is now a full escrowed marketplace:

- Listings enforce skin ownership, a seller-defined duration, and the rarity
  floor from `skins::rarity_floor_price`.
- Listing escrows the skin by clearing its `tradeable` flag; buying, cancelling,
  or expiring restores it.
- Listings embed the `SkinPreview` payload (gradient, accent, effect layers,
  thumbnail seed), so buyers can preview before purchase.
- Search supports max price and price/rarity filters over an all-listings index.
- Sellers can reprice active listings and query their listing history.
- Player-to-player barter offers escrow the offered skin and swap both skins
  atomically when accepted.
- Marketplace stats track total listings, active listings, settled volume, and
  average sale price.

## #644 — Content Creator Tools and Marketplace

The existing creation, approval, voting, revenue-split, purchase, and withdrawal
flows are retained. Added:

- Typed `create_nebula`, `create_mission`, and `create_event` helpers.
- Marketplace browsing across active listings and price search.
- Per-seller listing indexes with the existing marketplace listing cap.
- One active marketplace listing per content item.

## #645 — NFT Marketplace Integration

Added listing management and category-safe helpers on top of the existing
fixed-price, auction, bundle, offer, escrow, search, analytics, and fraud
features:

- Read a listing or a seller's listing history.
- Reprice an active fixed-price listing.
- Permissionless expiry sweep that releases escrow and updates counters.
- `create_ship_listing`, `create_skin_listing`, and `create_badge_listing`
  construct the correct `AssetCategory` so ships, skins, and badges always land
  in the right market filter.

## Verification note

Per the task instructions, no install, build, test, or lint command was run.
