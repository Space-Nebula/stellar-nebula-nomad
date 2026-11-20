#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{testutils::Address as _, Address, Env};

    fn make_env() -> Env {
        let env = Env::default();
        env.mock_all_auths();
        env
    }

    fn in_contract<T>(env: &Env, f: impl FnOnce() -> T) -> T {
        let contract = env.register(ResourceMinterContract, ());
        env.as_contract(&contract, f)
    }

    // ── Arithmetic safety (Issue #239) ──────────────────────────
    //
    // `mint_resource` credits balances via `current.checked_add(amount)`
    // (see above). These property tests pin down that operation's
    // overflow behavior directly, independent of the full mint flow.
    mod arithmetic_safety {
        use proptest::prelude::*;

        proptest! {
            #[test]
            fn checked_credit_never_wraps(current in any::<u64>(), amount in any::<u64>()) {
                match current.checked_add(amount) {
                    Some(sum) => {
                        prop_assert!(sum >= current);
                        prop_assert!(sum >= amount);
                    }
                    None => {
                        // Only reports overflow when the true (unbounded) sum
                        // would actually exceed u64::MAX — never a false positive.
                        prop_assert!(current as u128 + amount as u128 > u64::MAX as u128);
                    }
                }
            }
        }

        #[test]
        fn checked_credit_detects_overflow_at_max_balance() {
            assert_eq!(u64::MAX.checked_add(1), None);
            assert_eq!((u64::MAX - 1).checked_add(1), Some(u64::MAX));
        }
    }

    #[test]
    fn test_mint_zero_amount_rejected() {
        let env = make_env();
        let caller = Address::generate(&env);
        let result = in_contract(&env, || {
            ResourceMinterContract::mint_resource(&env, caller, 1, 0, ResourceType::StellarDust, 0)
        });
        assert_eq!(result, Err(MinterError::InvalidAmount));
    }

    #[test]
    fn test_mint_rejected_while_guard_held() {
        // Simulates a callback re-entering mint_resource while an earlier
        // guarded invocation is still in flight (Issue #472).
        let env = make_env();
        let caller = Address::generate(&env);
        let contract = env.register(ResourceMinterContract, ());

        env.as_contract(&contract, || {
            crate::reentrancy_guard::acquire(&env).expect("lock should be free");
            let result = ResourceMinterContract::mint_resource(
                &env,
                caller.clone(),
                1,
                0,
                ResourceType::StellarDust,
                5,
            );
            assert_eq!(result, Err(MinterError::Reentrancy));
            crate::reentrancy_guard::release(&env);

            // The rejected call minted nothing.
            assert_eq!(balance_of(&env, &caller, &ResourceType::StellarDust), 0);
            assert_eq!(circulating_supply(&env, &ResourceType::StellarDust), 0);
        });

        // With the lock released the call proceeds past the guard to validation.
        env.as_contract(&contract, || {
            let result = ResourceMinterContract::mint_resource(
                &env,
                caller.clone(),
                1,
                0,
                ResourceType::StellarDust,
                0,
            );
            assert_eq!(result, Err(MinterError::InvalidAmount));
        });
    }

    #[test]
    fn test_rate_limit_enforced_on_minting() {
        let env = make_env();
        let caller = Address::generate(&env);
        let contract = env.register(ResourceMinterContract, ());

        // Use up the default ResourceMinting limit (10 / 60 s).
        for _ in 0..10 {
            env.as_contract(&contract, || {
                let _ = ResourceMinterContract::mint_resource(
                    &env,
                    caller.clone(),
                    1,
                    0,
                    ResourceType::StellarDust,
                    1,
                );
            });
        }
        let result = env.as_contract(&contract, || {
            let result = ResourceMinterContract::mint_resource(
                &env,
                caller,
                1,
                0,
                ResourceType::StellarDust,
                1,
            );
            result
        });
        assert_eq!(result, Err(MinterError::RateLimitExceeded));
    }

    // ── Supply-reducing primitives (Issue #281) ─────────────────
    //
    // These touch the balance and supply ledgers, so — unlike the pure
    // arithmetic tests above — they must run inside a contract invocation.
    mod supply_primitives {
        use super::*;
        use soroban_sdk::contractimpl;

        #[contract]
        struct Stub;
        #[contractimpl]
        impl Stub {}

        fn in_contract<T>(f: impl FnOnce(&Env) -> T) -> T {
            let env = make_env();
            let contract = env.register(Stub, ());
            env.as_contract(&contract, || f(&env))
        }

        #[test]
        fn credit_updates_balance_supply_and_mint_total() {
            in_contract(|env| {
                let holder = Address::generate(env);
                let rt = ResourceType::DarkMatter;

                assert_eq!(credit_balance(env, &holder, &rt, 400).unwrap(), 400);
                assert_eq!(balance_of(env, &holder, &rt), 400);
                assert_eq!(circulating_supply(env, &rt), 400);
                assert_eq!(total_minted(env, &rt), 400);
            });
        }

        #[test]
        fn debit_reduces_balance_but_not_the_mint_total() {
            in_contract(|env| {
                let holder = Address::generate(env);
                let rt = ResourceType::StellarDust;
                credit_balance(env, &holder, &rt, 100).unwrap();

                assert_eq!(debit_balance(env, &holder, &rt, 40).unwrap(), 60);
                assert_eq!(reduce_supply(env, &rt, 40).unwrap(), 60);
                assert_eq!(
                    total_minted(env, &rt),
                    100,
                    "the historical mint total must be monotonic"
                );
            });
        }

        #[test]
        fn debit_beyond_balance_is_rejected_without_wrapping() {
            in_contract(|env| {
                let holder = Address::generate(env);
                let rt = ResourceType::StellarDust;
                credit_balance(env, &holder, &rt, 10).unwrap();

                assert_eq!(
                    debit_balance(env, &holder, &rt, 11),
                    Err(MinterError::InsufficientBalance)
                );
                assert_eq!(balance_of(env, &holder, &rt), 10, "balance is unchanged");
            });
        }

        #[test]
        fn reduce_supply_beyond_supply_is_rejected() {
            in_contract(|env| {
                assert_eq!(
                    reduce_supply(env, &ResourceType::ExoticMatter, 1),
                    Err(MinterError::InsufficientBalance)
                );
            });
        }

        #[test]
        fn zero_amount_transfers_are_rejected() {
            in_contract(|env| {
                let holder = Address::generate(env);
                let rt = ResourceType::StellarDust;

                assert_eq!(
                    credit_balance(env, &holder, &rt, 0),
                    Err(MinterError::InvalidAmount)
                );
                assert_eq!(
                    debit_balance(env, &holder, &rt, 0),
                    Err(MinterError::InvalidAmount)
                );
            });
        }

        #[test]
        fn credit_detects_balance_overflow() {
            in_contract(|env| {
                let holder = Address::generate(env);
                let rt = ResourceType::StellarDust;
                credit_balance(env, &holder, &rt, u64::MAX).unwrap();

                assert_eq!(
                    credit_balance(env, &holder, &rt, 1),
                    Err(MinterError::ArithmeticOverflow)
                );
            });
        }

        #[test]
        fn balances_are_tracked_per_resource_type() {
            in_contract(|env| {
                let holder = Address::generate(env);
                credit_balance(env, &holder, &ResourceType::StellarDust, 10).unwrap();
                credit_balance(env, &holder, &ResourceType::DarkMatter, 20).unwrap();

                assert_eq!(balance_of(env, &holder, &ResourceType::StellarDust), 10);
                assert_eq!(balance_of(env, &holder, &ResourceType::DarkMatter), 20);
                assert_eq!(balance_of(env, &holder, &ResourceType::ExoticMatter), 0);
            });
        }

        #[test]
        fn move_balance_shifts_holdings_without_changing_supply() {
            in_contract(|env| {
                let from = Address::generate(env);
                let to = Address::generate(env);
                let rt = ResourceType::DarkMatter;
                credit_balance(env, &from, &rt, 500).unwrap();

                assert_eq!(move_balance(env, &from, &to, &rt, 200).unwrap(), 200);
                assert_eq!(balance_of(env, &from, &rt), 300);
                assert_eq!(balance_of(env, &to, &rt), 200);
                assert_eq!(circulating_supply(env, &rt), 500, "a move is not a mint");
                assert_eq!(total_minted(env, &rt), 500);
            });
        }

        #[test]
        fn move_balance_rejects_an_underfunded_sender() {
            in_contract(|env| {
                let from = Address::generate(env);
                let to = Address::generate(env);
                let rt = ResourceType::DarkMatter;
                credit_balance(env, &from, &rt, 10).unwrap();

                assert_eq!(
                    move_balance(env, &from, &to, &rt, 11),
                    Err(MinterError::InsufficientBalance)
                );
                assert_eq!(balance_of(env, &from, &rt), 10);
                assert_eq!(balance_of(env, &to, &rt), 0);
            });
        }

        #[test]
        fn move_balance_to_self_is_a_no_op() {
            in_contract(|env| {
                let holder = Address::generate(env);
                let rt = ResourceType::DarkMatter;
                credit_balance(env, &holder, &rt, 100).unwrap();

                assert_eq!(move_balance(env, &holder, &holder, &rt, 50).unwrap(), 100);
                assert_eq!(balance_of(env, &holder, &rt), 100);
            });
        }
    }

    // ── Harvest + DEX auto-listing ─────────────────────────────────
    mod harvest_and_dex {
        use super::*;
        use crate::nebula_explorer::{CellType, NebulaCell};
        use soroban_sdk::contractimpl;
        use soroban_sdk::testutils::{Events as _, Ledger, LedgerInfo};

        #[contract]
        struct Stub;

        #[contractimpl]
        impl Stub {}

        /// Run `f` as a single contract invocation.
        ///
        /// `mock_all_auths` rejects a second `require_auth()` for the same
        /// address within one invocation, so a test that legitimately performs
        /// several authorized contract calls must use a fresh `as_contract` block
        /// per call (via [`Contract`]) rather than nesting them here.
        fn in_contract<T>(f: impl FnOnce(&Env) -> T) -> T {
            let env = make_env();
            let contract = env.register(Stub, ());
            env.as_contract(&contract, || f(&env))
        }

        /// A registered contract whose invocations can be driven one at a time.
        struct Contract {
            env: Env,
            id: soroban_sdk::Address,
        }

        impl Contract {
            fn new() -> Self {
                let env = make_env();
                let id = env.register(Stub, ());
                Self { env, id }
            }

            fn env(&self) -> &Env {
                &self.env
            }

            /// Perform one contract invocation.
            fn invoke<T>(&self, f: impl FnOnce(&Env) -> T) -> T {
                self.env.as_contract(&self.id, || f(&self.env))
            }
        }

        /// Build a single-cell layout with the given cell type and energy.
        fn layout_with(env: &Env, cell_type: CellType, energy: u32) -> NebulaLayout {
            let mut cells = Vec::new(env);
            cells.push_back(NebulaCell {
                x: 0,
                y: 0,
                cell_type,
                energy,
            });
            NebulaLayout {
                width: 1,
                height: 1,
                cells,
                seed: soroban_sdk::BytesN::from_array(env, &[7u8; 32]),
                timestamp: 0,
                total_energy: energy,
            }
        }

        /// `Events::all()` reads back the host's ledger event log, so a ledger
        /// must exist before any event assertion.
        fn seed_ledger(env: &Env) {
            env.ledger().set(LedgerInfo {
                protocol_version: 28,
                sequence_number: 0,
                timestamp: 0,
                network_id: [0u8; 32],
                base_reserve: 10,
                min_temp_entry_ttl: 100,
                min_persistent_entry_ttl: 100,
                max_entry_ttl: 1_000,
            });
        }

        /// Register a ship owned by `owner` and return its ID.
        fn ship_for(env: &Env, owner: &Address) -> u64 {
            crate::ship_nft::mint_ship(
                env,
                owner,
                &soroban_sdk::symbol_short!("explorer"),
                &soroban_sdk::Bytes::new(env),
            )
            .unwrap()
            .id
        }

        // ── cell_type_to_asset ───────────────────────────────────

        #[test]
        fn resource_cells_map_to_an_asset() {
            for ct in [
                CellType::StellarDust,
                CellType::Asteroid,
                CellType::GasCloud,
                CellType::DarkMatter,
                CellType::ExoticMatter,
                CellType::Wormhole,
            ] {
                assert!(
                    cell_type_to_asset(&ct).is_some(),
                    "{ct:?} should be harvestable"
                );
            }
        }

        #[test]
        fn empty_and_star_cells_map_to_no_asset() {
            assert!(cell_type_to_asset(&CellType::Empty).is_none());
            assert!(cell_type_to_asset(&CellType::Star).is_none());
        }

        // ── harvest_resources ────────────────────────────────────

        #[test]
        fn harvest_credits_the_ship_owner() {
            in_contract(|env| {
                let owner = Address::generate(env);
                let ship_id = ship_for(env, &owner);
                let layout = layout_with(env, CellType::StellarDust, 40);

                let result = harvest_resources(env, ship_id, &layout).unwrap();

                assert_eq!(result.ship_id, ship_id);
                assert_eq!(result.total_harvested, 40);
                assert_eq!(result.resources.len(), 1);
                assert_eq!(
                    resource_balance(env, &owner, &soroban_sdk::symbol_short!("dust")),
                    40
                );
            });
        }

        #[test]
        fn harvest_credits_the_current_owner_after_transfer() {
            let c = Contract::new();
            let env = c.env();
            let from = Address::generate(env);
            let to = Address::generate(env);

            let ship_id = c.invoke(|env| ship_for(env, &from));
            c.invoke(|env| crate::ship_nft::transfer_ship(env, ship_id, &from, &to).unwrap());

            let layout = layout_with(env, CellType::DarkMatter, 12);
            c.invoke(|env| harvest_resources(env, ship_id, &layout).unwrap());

            // The new owner is credited, not the one who minted the ship.
            c.invoke(|env| {
                assert_eq!(
                    resource_balance(env, &to, &soroban_sdk::symbol_short!("dark")),
                    12
                );
                assert_eq!(
                    resource_balance(env, &from, &soroban_sdk::symbol_short!("dark")),
                    0
                );
            });
        }

        #[test]
        fn harvest_rejects_an_unknown_ship() {
            in_contract(|env| {
                let layout = layout_with(env, CellType::Asteroid, 5);
                assert_eq!(
                    harvest_resources(env, 9_999, &layout),
                    Err(HarvestError::ShipNotFound)
                );
            });
        }

        #[test]
        fn harvest_rejects_a_layout_with_no_resources() {
            in_contract(|env| {
                let owner = Address::generate(env);
                let ship_id = ship_for(env, &owner);
                let layout = layout_with(env, CellType::Empty, 100);

                assert_eq!(
                    harvest_resources(env, ship_id, &layout),
                    Err(HarvestError::EmptyHarvest)
                );
            });
        }

        #[test]
        fn harvest_ignores_zero_energy_cells() {
            in_contract(|env| {
                let owner = Address::generate(env);
                let ship_id = ship_for(env, &owner);
                let layout = layout_with(env, CellType::ExoticMatter, 0);

                assert_eq!(
                    harvest_resources(env, ship_id, &layout),
                    Err(HarvestError::EmptyHarvest)
                );
            });
        }

        #[test]
        fn harvest_accumulates_across_calls() {
            in_contract(|env| {
                let owner = Address::generate(env);
                let ship_id = ship_for(env, &owner);
                let layout = layout_with(env, CellType::GasCloud, 3);

                harvest_resources(env, ship_id, &layout).unwrap();
                harvest_resources(env, ship_id, &layout).unwrap();

                assert_eq!(
                    resource_balance(env, &owner, &soroban_sdk::symbol_short!("gas")),
                    6
                );
            });
        }

        #[test]
        fn harvest_separates_assets_by_type() {
            in_contract(|env| {
                let owner = Address::generate(env);
                let ship_id = ship_for(env, &owner);

                let mut cells = Vec::new(env);
                cells.push_back(NebulaCell {
                    x: 0,
                    y: 0,
                    cell_type: CellType::StellarDust,
                    energy: 10,
                });
                cells.push_back(NebulaCell {
                    x: 1,
                    y: 0,
                    cell_type: CellType::DarkMatter,
                    energy: 25,
                });
                // Non-resource cell must be skipped without affecting the total.
                cells.push_back(NebulaCell {
                    x: 2,
                    y: 0,
                    cell_type: CellType::Star,
                    energy: 500,
                });
                let layout = NebulaLayout {
                    width: 3,
                    height: 1,
                    cells,
                    seed: soroban_sdk::BytesN::from_array(env, &[1u8; 32]),
                    timestamp: 0,
                    total_energy: 535,
                };

                let result = harvest_resources(env, ship_id, &layout).unwrap();

                assert_eq!(result.resources.len(), 2);
                assert_eq!(result.total_harvested, 35);
                assert_eq!(
                    resource_balance(env, &owner, &soroban_sdk::symbol_short!("dust")),
                    10
                );
                assert_eq!(
                    resource_balance(env, &owner, &soroban_sdk::symbol_short!("dark")),
                    25
                );
            });
        }

        #[test]
        fn harvest_emits_an_event() {
            in_contract(|env| {
                seed_ledger(env);
                let owner = Address::generate(env);
                let ship_id = ship_for(env, &owner);
                let layout = layout_with(env, CellType::Wormhole, 8);
                let before = env.events().all().events().len();

                harvest_resources(env, ship_id, &layout).unwrap();

                assert_eq!(env.events().all().events().len(), before + 1);
            });
        }

        // ── auto_list_on_dex ─────────────────────────────────────

        #[test]
        fn auto_list_creates_an_active_offer() {
            in_contract(|env| {
                let seller = Address::generate(env);
                let asset = soroban_sdk::symbol_short!("dust");
                credit_resource_balance(env, &seller, &asset, 60).unwrap();

                let offer = auto_list_on_dex(env, &seller, &asset, 25).unwrap();

                assert_eq!(offer.offer_id, 1);
                assert_eq!(offer.asset_id, asset);
                assert_eq!(offer.amount, 60);
                assert_eq!(offer.min_price, 25);
                assert!(offer.active);
            });
        }

        #[test]
        fn auto_list_escrows_the_listed_balance() {
            in_contract(|env| {
                let seller = Address::generate(env);
                let asset = soroban_sdk::symbol_short!("dust");
                credit_resource_balance(env, &seller, &asset, 60).unwrap();

                auto_list_on_dex(env, &seller, &asset, 25).unwrap();

                // Escrowed, so it cannot also be spent on a gift or burned.
                assert_eq!(resource_balance(env, &seller, &asset), 0);
            });
        }

        #[test]
        fn auto_list_rejects_a_non_positive_price() {
            let c = Contract::new();
            let env = c.env();
            let asset = soroban_sdk::symbol_short!("dust");
            let seller = Address::generate(env);
            c.invoke(|env| credit_resource_balance(env, &seller, &asset, 10).unwrap());

            assert_eq!(
                c.invoke(|env| auto_list_on_dex(env, &seller, &asset, 0)),
                Err(HarvestError::InvalidPrice)
            );
            assert_eq!(
                c.invoke(|env| auto_list_on_dex(env, &seller, &asset, -1)),
                Err(HarvestError::InvalidPrice)
            );

            // Nothing was escrowed by a rejected listing.
            c.invoke(|env| assert_eq!(resource_balance(env, &seller, &asset), 10));
        }

        #[test]
        fn auto_list_rejects_an_empty_balance() {
            in_contract(|env| {
                let seller = Address::generate(env);
                let asset = soroban_sdk::symbol_short!("dust");

                assert_eq!(
                    auto_list_on_dex(env, &seller, &asset, 5),
                    Err(HarvestError::InsufficientBalance)
                );
            });
        }

        #[test]
        fn offer_ids_are_unique_and_monotonic() {
            in_contract(|env| {
                let a = Address::generate(env);
                let b = Address::generate(env);
                let asset = soroban_sdk::symbol_short!("dust");
                credit_resource_balance(env, &a, &asset, 10).unwrap();
                credit_resource_balance(env, &b, &asset, 10).unwrap();

                let first = auto_list_on_dex(env, &a, &asset, 1).unwrap();
                let second = auto_list_on_dex(env, &b, &asset, 1).unwrap();

                assert_eq!(first.offer_id, 1);
                assert_eq!(second.offer_id, 2);
                assert_ne!(first.offer_id, second.offer_id);
            });
        }

        #[test]
        fn get_dex_offer_round_trips() {
            in_contract(|env| {
                let seller = Address::generate(env);
                let asset = soroban_sdk::symbol_short!("dark");
                credit_resource_balance(env, &seller, &asset, 7).unwrap();

                let offer = auto_list_on_dex(env, &seller, &asset, 3).unwrap();

                assert_eq!(get_dex_offer(env, offer.offer_id), Some(offer));
                assert_eq!(get_dex_offer(env, 4_242), None);
            });
        }

        #[test]
        fn auto_list_emits_an_event() {
            in_contract(|env| {
                seed_ledger(env);
                let seller = Address::generate(env);
                let asset = soroban_sdk::symbol_short!("dust");
                credit_resource_balance(env, &seller, &asset, 4).unwrap();
                let before = env.events().all().events().len();

                auto_list_on_dex(env, &seller, &asset, 2).unwrap();

                assert_eq!(env.events().all().events().len(), before + 1);
            });
        }

        // ── Reentrancy protection (Issue #472) ──────────────────

        #[test]
        fn harvest_is_rejected_while_guard_held() {
            let c = Contract::new();
            let env = c.env();
            let owner = Address::generate(env);
            let ship_id = c.invoke(|env| ship_for(env, &owner));
            let layout = layout_with(env, CellType::StellarDust, 40);
            let dust = soroban_sdk::symbol_short!("dust");

            c.invoke(|env| {
                crate::reentrancy_guard::acquire(env).expect("lock should be free");
                assert_eq!(
                    harvest_resources(env, ship_id, &layout),
                    Err(HarvestError::Reentrancy)
                );
                crate::reentrancy_guard::release(env);
                // The rejected harvest credited nothing.
                assert_eq!(resource_balance(env, &owner, &dust), 0);
            });

            c.invoke(|env| {
                harvest_resources(env, ship_id, &layout).unwrap();
                assert_eq!(resource_balance(env, &owner, &dust), 40);
                assert!(!crate::reentrancy_guard::is_locked(env));
            });
        }

        #[test]
        fn auto_list_is_rejected_while_guard_held() {
            in_contract(|env| {
                let seller = Address::generate(env);
                let asset = soroban_sdk::symbol_short!("dust");
                credit_resource_balance(env, &seller, &asset, 60).unwrap();

                crate::reentrancy_guard::acquire(env).expect("lock should be free");
                assert_eq!(
                    auto_list_on_dex(env, &seller, &asset, 25),
                    Err(HarvestError::Reentrancy)
                );
                crate::reentrancy_guard::release(env);

                // No escrow was taken and no offer was created.
                assert_eq!(resource_balance(env, &seller, &asset), 60);
                assert_eq!(get_dex_offer(env, 1), None);

                let offer = auto_list_on_dex(env, &seller, &asset, 25).unwrap();
                assert_eq!(offer.amount, 60);
            });
        }

        #[test]
        fn harvest_and_list_rejects_reentry_but_composes_its_own_harvest() {
            let c = Contract::new();
            let env = c.env();
            let owner = Address::generate(env);
            let ship_id = c.invoke(|env| ship_for(env, &owner));
            let layout = layout_with(env, CellType::StellarDust, 30);
            let asset = soroban_sdk::symbol_short!("dust");

            c.invoke(|env| {
                crate::reentrancy_guard::acquire(env).expect("lock should be free");
                assert_eq!(
                    crate::dex_integration::harvest_and_list(
                        env, &owner, ship_id, &layout, &asset, 5
                    )
                    .map(|(_, offer)| offer.offer_id),
                    Err(HarvestError::Reentrancy)
                );
                crate::reentrancy_guard::release(env);
                assert_eq!(resource_balance(env, &owner, &asset), 0);
            });

            // Unlocked, the harvest leg runs inside the same guard without
            // tripping it, and the lock is released afterwards.
            let (harvest, offer) = c.invoke(|env| {
                crate::dex_integration::harvest_and_list(env, &owner, ship_id, &layout, &asset, 5)
                    .unwrap()
            });
            assert_eq!(harvest.total_harvested, 30);
            assert_eq!(offer.amount, 30);
            c.invoke(|env| assert!(!crate::reentrancy_guard::is_locked(env)));
        }

        #[test]
        fn cancel_listing_is_rejected_while_guard_held() {
            let c = Contract::new();
            let env = c.env();
            let seller = Address::generate(env);
            let asset = soroban_sdk::symbol_short!("dust");
            let offer = c.invoke(|env| {
                credit_resource_balance(env, &seller, &asset, 9).unwrap();
                auto_list_on_dex(env, &seller, &asset, 4).unwrap()
            });

            c.invoke(|env| {
                crate::reentrancy_guard::acquire(env).expect("lock should be free");
                assert_eq!(
                    crate::dex_integration::cancel_listing(env, &seller, offer.offer_id),
                    Err(HarvestError::Reentrancy)
                );
                crate::reentrancy_guard::release(env);
                // Offer is still live and the escrow was not refunded.
                assert!(get_dex_offer(env, offer.offer_id).unwrap().active);
                assert_eq!(resource_balance(env, &seller, &asset), 0);
            });

            c.invoke(|env| {
                crate::dex_integration::cancel_listing(env, &seller, offer.offer_id).unwrap();
                assert_eq!(resource_balance(env, &seller, &asset), 9);
            });
        }

        // ── credit overflow (Issue #239) ────────────────────────

        #[test]
        fn crediting_past_u32_max_overflows_instead_of_wrapping() {
            in_contract(|env| {
                let owner = Address::generate(env);
                let asset = soroban_sdk::symbol_short!("dust");
                credit_resource_balance(env, &owner, &asset, u32::MAX).unwrap();

                assert_eq!(
                    credit_resource_balance(env, &owner, &asset, 1),
                    Err(HarvestError::PriceOverflow)
                );
                // Balance must be untouched after the rejected credit.
                assert_eq!(resource_balance(env, &owner, &asset), u32::MAX);
            });
        }
    }
}

#[cfg(test)]
mod packed_record_tests {
    use super::*;
    use soroban_sdk::testutils::Address as _;

    #[test]
    fn packed_resource_record_round_trips() {
        let env = Env::default();
        let rec = ResourceRecord {
            owner: Address::generate(&env),
            resource_type: ResourceType::DarkMatter,
            amount: u64::MAX,
            minted_at: 1_700_000_000,
        };
        let packed = PackedResourceRecord::from(&rec);
        assert_eq!(packed.packed, (1_700_000_000u128 << 64) | u64::MAX as u128);
        assert_eq!(ResourceRecord::from(&packed), rec);
    }
}
