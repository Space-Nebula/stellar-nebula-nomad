#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::testutils::{Address as _, Ledger};
    use soroban_sdk::{contract, contractimpl};

    #[contract]
    struct Stub;
    #[contractimpl]
    impl Stub {}

    /// Quest state lives in contract storage, so test bodies run through
    /// `Env::as_contract`. Each `run` is a separate invocation frame, matching
    /// how the flow works in practice (start, progress, claim and branch are
    /// distinct transactions) and satisfying the rule that `require_auth` may
    /// only be met once per frame.
    struct Fixture {
        env: Env,
        contract: Address,
        author: Address,
        player: Address,
    }

    impl Fixture {
        fn run<T>(&self, f: impl FnOnce() -> T) -> T {
            self.env.as_contract(&self.contract, f)
        }
    }

    fn fixture() -> Fixture {
        let f = bare_fixture();
        let player = f.player.clone();
        f.run(|| player_profile::initialize_profile(&f.env, player).unwrap());
        f
    }

    /// Like [`fixture`] but the player has no profile, so rewards cannot land.
    fn bare_fixture() -> Fixture {
        let env = Env::default();
        env.mock_all_auths();
        let contract = env.register(Stub, ());
        let author = Address::generate(&env);
        let player = Address::generate(&env);
        Fixture {
            env,
            contract,
            author,
            player,
        }
    }

    fn reward(essence: i128) -> QuestReward {
        QuestReward {
            essence,
            resource_type: ResourceType::StellarDust,
            resource_amount: 0,
            xp: 10,
            cosmetic_roll: false,
        }
    }

    fn branch(env: &Env, choice_id: u32, label: &str, next: u64) -> QuestBranch {
        QuestBranch {
            choice_id,
            label: Symbol::new(env, label),
            next_quest_id: next,
        }
    }

    fn branches(env: &Env, items: &[QuestBranch]) -> Vec<QuestBranch> {
        let mut v = Vec::new(env);
        for b in items {
            v.push_back(b.clone());
        }
        v
    }

    /// Add a node with sensible defaults for the fields a test does not care
    /// about, so call sites stay readable.
    fn add_node(
        f: &Fixture,
        chain_id: u64,
        objective: Symbol,
        target: u32,
        essence: i128,
        node_branches: Vec<QuestBranch>,
        duration_secs: u64,
    ) -> Result<u64, QuestError> {
        f.run(|| {
            add_quest_node(
                &f.env,
                f.author.clone(),
                chain_id,
                objective,
                target,
                reward(essence),
                symbol_short!("tier"),
                String::from_str(&f.env, "Title"),
                String::from_str(&f.env, "Description"),
                node_branches,
                duration_secs,
            )
        })
    }

    /// A three-node chain that branches after the root:
    ///
    /// ```text
    ///   node1 ─┬─ choice 1 ─► node2 ─► (end)
    ///          └─ choice 2 ─► node3 ─► (end)
    /// ```
    fn branching_chain(f: &Fixture) -> (u64, u64, u64, u64) {
        let chain_id =
            f.run(|| define_chain(&f.env, f.author.clone(), symbol_short!("prologue")).unwrap());

        // The root is added first (that is what makes it the root) and carries
        // forward references to nodes 2 and 3, which do not exist yet.
        let node1 = add_node(
            f,
            chain_id,
            symbol_short!("scan"),
            3,
            100,
            branches(
                &f.env,
                &[branch(&f.env, 1, "ally", 2), branch(&f.env, 2, "raid", 3)],
            ),
            0,
        )
        .unwrap();

        let node2 = add_node(
            f,
            chain_id,
            symbol_short!("trade"),
            2,
            250,
            branches(&f.env, &[branch(&f.env, 1, "finish", CHAIN_TERMINUS)]),
            0,
        )
        .unwrap();

        let node3 = add_node(
            f,
            chain_id,
            symbol_short!("harvest"),
            5,
            400,
            Vec::new(&f.env),
            0,
        )
        .unwrap();

        (chain_id, node1, node2, node3)
    }

    // ── Authoring ─────────────────────────────────────────────────────────

    #[test]
    fn chain_records_its_root_and_node_count() {
        let f = fixture();
        let (chain_id, node1, _, _) = branching_chain(&f);

        f.run(|| {
            let chain = get_chain(&f.env, chain_id).unwrap();
            assert_eq!(chain.root_quest_id, node1);
            assert_eq!(chain.node_count, 3);
            assert_eq!(chain.creator, f.author);
        });
    }

    #[test]
    fn node_step_indexes_follow_insertion_order() {
        let f = fixture();
        let (_, node1, node2, node3) = branching_chain(&f);

        f.run(|| {
            assert_eq!(get_quest_node(&f.env, node1).unwrap().step_index, 0);
            assert_eq!(get_quest_node(&f.env, node2).unwrap().step_index, 1);
            assert_eq!(get_quest_node(&f.env, node3).unwrap().step_index, 2);
        });
    }

    #[test]
    fn only_the_creator_may_extend_a_chain() {
        let f = fixture();
        let (chain_id, _, _, _) = branching_chain(&f);

        let result = f.run(|| {
            add_quest_node(
                &f.env,
                f.player.clone(),
                chain_id,
                symbol_short!("scan"),
                1,
                reward(1),
                symbol_short!("tier"),
                String::from_str(&f.env, "t"),
                String::from_str(&f.env, "d"),
                Vec::new(&f.env),
                0,
            )
        });
        assert_eq!(result, Err(QuestError::Unauthorized));
    }

    #[test]
    fn node_rejects_zero_target_and_zero_choice_ids() {
        let f = fixture();
        let chain_id =
            f.run(|| define_chain(&f.env, f.author.clone(), symbol_short!("c")).unwrap());

        assert_eq!(
            add_node(
                &f,
                chain_id,
                symbol_short!("scan"),
                0,
                1,
                Vec::new(&f.env),
                0
            ),
            Err(QuestError::InvalidTarget)
        );
        assert_eq!(
            add_node(
                &f,
                chain_id,
                symbol_short!("scan"),
                1,
                1,
                branches(&f.env, &[branch(&f.env, 0, "bad", CHAIN_TERMINUS)]),
                0
            ),
            Err(QuestError::InvalidBranch)
        );
    }

    #[test]
    fn node_rejects_too_many_branches() {
        let f = fixture();
        let chain_id =
            f.run(|| define_chain(&f.env, f.author.clone(), symbol_short!("c")).unwrap());

        let mut too_many = Vec::new(&f.env);
        for i in 1..=(MAX_BRANCHES_PER_NODE + 1) {
            too_many.push_back(branch(&f.env, i, "b", CHAIN_TERMINUS));
        }

        assert_eq!(
            add_node(&f, chain_id, symbol_short!("scan"), 1, 1, too_many, 0),
            Err(QuestError::TooManyBranches)
        );
    }

    #[test]
    fn chain_length_is_capped() {
        let f = fixture();
        let chain_id =
            f.run(|| define_chain(&f.env, f.author.clone(), symbol_short!("long")).unwrap());

        for _ in 0..MAX_CHAIN_LENGTH {
            add_node(
                &f,
                chain_id,
                symbol_short!("scan"),
                1,
                1,
                Vec::new(&f.env),
                0,
            )
            .unwrap();
        }

        assert_eq!(
            add_node(
                &f,
                chain_id,
                symbol_short!("scan"),
                1,
                1,
                Vec::new(&f.env),
                0
            ),
            Err(QuestError::ChainFull)
        );
    }

    #[test]
    fn adding_a_node_to_a_missing_chain_fails() {
        let f = fixture();
        assert_eq!(
            add_node(&f, 999, symbol_short!("scan"), 1, 1, Vec::new(&f.env), 0),
            Err(QuestError::ChainNotFound)
        );
    }

    #[test]
    fn validate_chain_accepts_a_complete_chain_and_rejects_dangling_edges() {
        let f = fixture();
        let (chain_id, _, _, _) = branching_chain(&f);
        f.run(|| validate_chain(&f.env, chain_id).unwrap());

        // A chain whose only node points at a node that was never authored.
        let broken =
            f.run(|| define_chain(&f.env, f.author.clone(), symbol_short!("broken")).unwrap());
        add_node(
            &f,
            broken,
            symbol_short!("scan"),
            1,
            1,
            branches(&f.env, &[branch(&f.env, 1, "nowhere", 9_999)]),
            0,
        )
        .unwrap();

        f.run(|| {
            assert_eq!(
                validate_chain(&f.env, broken),
                Err(QuestError::DanglingBranch)
            );
            assert_eq!(
                validate_chain(&f.env, 12_345),
                Err(QuestError::ChainNotFound)
            );
        });
    }

    // ── Starting ──────────────────────────────────────────────────────────

    #[test]
    fn starting_a_chain_activates_the_root_node() {
        let f = fixture();
        let (chain_id, node1, _, _) = branching_chain(&f);

        let state = f.run(|| start_chain(&f.env, f.player.clone(), chain_id).unwrap());
        assert_eq!(state.quest_id, node1);
        assert_eq!(state.status, QuestStatus::Active);
        assert_eq!(state.target, 3);
        assert_eq!(state.progress, 0);

        f.run(|| {
            assert_eq!(get_active_quests(&f.env, &f.player).len(), 1);
            let progress = get_chain_progress(&f.env, &f.player, chain_id).unwrap();
            assert_eq!(progress.current_quest_id, node1);
            assert_eq!(progress.steps_completed, 0);
            assert!(!progress.finished);
        });
    }

    #[test]
    fn a_chain_cannot_be_started_twice() {
        let f = fixture();
        let (chain_id, _, _, _) = branching_chain(&f);
        f.run(|| start_chain(&f.env, f.player.clone(), chain_id).unwrap());

        f.run(|| {
            assert_eq!(
                start_chain(&f.env, f.player.clone(), chain_id),
                Err(QuestError::ChainAlreadyStarted)
            );
        });
    }

    #[test]
    fn starting_an_unknown_or_empty_chain_fails() {
        let f = fixture();
        f.run(|| {
            assert_eq!(
                start_chain(&f.env, f.player.clone(), 42),
                Err(QuestError::ChainNotFound)
            );
        });

        let empty =
            f.run(|| define_chain(&f.env, f.author.clone(), symbol_short!("empty")).unwrap());
        f.run(|| {
            assert_eq!(
                start_chain(&f.env, f.player.clone(), empty),
                Err(QuestError::QuestNotFound)
            );
        });
    }

    #[test]
    fn active_quests_are_capped_per_player() {
        let f = fixture();

        // Each single-node chain contributes one active quest.
        for _ in 0..MAX_ACTIVE_QUESTS {
            let chain_id =
                f.run(|| define_chain(&f.env, f.author.clone(), symbol_short!("c")).unwrap());
            add_node(
                &f,
                chain_id,
                symbol_short!("scan"),
                1,
                1,
                Vec::new(&f.env),
                0,
            )
            .unwrap();
            f.run(|| start_chain(&f.env, f.player.clone(), chain_id).unwrap());
        }

        let extra =
            f.run(|| define_chain(&f.env, f.author.clone(), symbol_short!("extra")).unwrap());
        add_node(&f, extra, symbol_short!("scan"), 1, 1, Vec::new(&f.env), 0).unwrap();
        f.run(|| {
            assert_eq!(
                start_chain(&f.env, f.player.clone(), extra),
                Err(QuestError::TooManyActiveQuests)
            );
        });
    }

    // ── Progress tracking ─────────────────────────────────────────────────

    #[test]
    fn progress_accumulates_then_completes_at_the_target() {
        let f = fixture();
        let (chain_id, node1, _, _) = branching_chain(&f);
        f.run(|| start_chain(&f.env, f.player.clone(), chain_id).unwrap());

        let s1 = f.run(|| record_progress(&f.env, f.player.clone(), node1, 1).unwrap());
        assert_eq!(s1.progress, 1);
        assert_eq!(s1.status, QuestStatus::Active);

        let s2 = f.run(|| record_progress(&f.env, f.player.clone(), node1, 2).unwrap());
        assert_eq!(s2.progress, 3);
        assert_eq!(s2.status, QuestStatus::Completed);
        assert_eq!(s2.completed_at, f.env.ledger().timestamp());

        // Completing removes the quest from the active list.
        f.run(|| assert_eq!(get_active_quests(&f.env, &f.player).len(), 0));
    }

    #[test]
    fn progress_is_clamped_to_the_target() {
        let f = fixture();
        let (chain_id, node1, _, _) = branching_chain(&f);
        f.run(|| start_chain(&f.env, f.player.clone(), chain_id).unwrap());

        let state = f.run(|| record_progress(&f.env, f.player.clone(), node1, 99).unwrap());
        assert_eq!(state.progress, 3, "progress never exceeds the target");
    }

    #[test]
    fn progress_on_a_completed_quest_is_rejected() {
        let f = fixture();
        let (chain_id, node1, _, _) = branching_chain(&f);
        f.run(|| start_chain(&f.env, f.player.clone(), chain_id).unwrap());
        f.run(|| record_progress(&f.env, f.player.clone(), node1, 3).unwrap());

        f.run(|| {
            assert_eq!(
                record_progress(&f.env, f.player.clone(), node1, 1),
                Err(QuestError::InvalidStatus)
            );
        });
    }

    #[test]
    fn progress_on_an_unstarted_quest_is_rejected() {
        let f = fixture();
        let (_, node1, _, _) = branching_chain(&f);
        f.run(|| {
            assert_eq!(
                record_progress(&f.env, f.player.clone(), node1, 1),
                Err(QuestError::QuestNotStarted)
            );
        });
    }

    #[test]
    fn a_quest_past_its_deadline_expires() {
        let f = fixture();
        let chain_id =
            f.run(|| define_chain(&f.env, f.author.clone(), symbol_short!("timed")).unwrap());
        let node = add_node(
            &f,
            chain_id,
            symbol_short!("scan"),
            5,
            10,
            Vec::new(&f.env),
            3_600,
        )
        .unwrap();
        f.run(|| start_chain(&f.env, f.player.clone(), chain_id).unwrap());

        f.env.ledger().with_mut(|l| l.timestamp += 3_601);

        f.run(|| {
            assert_eq!(
                record_progress(&f.env, f.player.clone(), node, 1),
                Err(QuestError::QuestExpired)
            );
        });
        f.run(|| {
            let state = get_quest_state(&f.env, &f.player, node).unwrap();
            assert_eq!(state.status, QuestStatus::Expired);
            assert_eq!(get_active_quests(&f.env, &f.player).len(), 0);
        });
    }

    // ── Rewards ───────────────────────────────────────────────────────────

    #[test]
    fn claiming_pays_essence_and_marks_the_quest_claimed() {
        let f = fixture();
        let (chain_id, node1, _, _) = branching_chain(&f);
        f.run(|| start_chain(&f.env, f.player.clone(), chain_id).unwrap());
        f.run(|| record_progress(&f.env, f.player.clone(), node1, 3).unwrap());

        let before = f.run(|| {
            player_profile::get_profile_by_owner(&f.env, &f.player)
                .unwrap()
                .essence_earned
        });
        let reward = f.run(|| claim_quest_reward(&f.env, f.player.clone(), node1).unwrap());
        let after = f.run(|| {
            player_profile::get_profile_by_owner(&f.env, &f.player)
                .unwrap()
                .essence_earned
        });

        assert_eq!(reward.essence, 100);
        assert_eq!(after - before, 100);
        f.run(|| {
            assert_eq!(
                get_quest_state(&f.env, &f.player, node1).unwrap().status,
                QuestStatus::Claimed
            );
        });
    }

    #[test]
    fn claiming_grants_resource_rewards() {
        let f = fixture();
        let chain_id =
            f.run(|| define_chain(&f.env, f.author.clone(), symbol_short!("res")).unwrap());
        let node = f
            .run(|| {
                add_quest_node(
                    &f.env,
                    f.author.clone(),
                    chain_id,
                    symbol_short!("harvest"),
                    1,
                    QuestReward {
                        essence: 0,
                        resource_type: ResourceType::DarkMatter,
                        resource_amount: 75,
                        xp: 5,
                        cosmetic_roll: true,
                    },
                    symbol_short!("tier"),
                    String::from_str(&f.env, "t"),
                    String::from_str(&f.env, "d"),
                    Vec::new(&f.env),
                    0,
                )
            })
            .unwrap();
        f.run(|| start_chain(&f.env, f.player.clone(), chain_id).unwrap());
        f.run(|| record_progress(&f.env, f.player.clone(), node, 1).unwrap());

        let reward = f.run(|| claim_quest_reward(&f.env, f.player.clone(), node).unwrap());
        assert!(reward.cosmetic_roll);
        f.run(|| {
            assert_eq!(
                resource_minter::balance_of(&f.env, &f.player, &ResourceType::DarkMatter),
                75
            );
        });
    }

    #[test]
    fn a_reward_cannot_be_claimed_twice() {
        let f = fixture();
        let (chain_id, node1, _, _) = branching_chain(&f);
        f.run(|| start_chain(&f.env, f.player.clone(), chain_id).unwrap());
        f.run(|| record_progress(&f.env, f.player.clone(), node1, 3).unwrap());
        f.run(|| claim_quest_reward(&f.env, f.player.clone(), node1).unwrap());

        f.run(|| {
            assert_eq!(
                claim_quest_reward(&f.env, f.player.clone(), node1),
                Err(QuestError::AlreadyClaimed)
            );
        });
    }

    #[test]
    fn an_incomplete_quest_cannot_be_claimed() {
        let f = fixture();
        let (chain_id, node1, _, _) = branching_chain(&f);
        f.run(|| start_chain(&f.env, f.player.clone(), chain_id).unwrap());
        f.run(|| record_progress(&f.env, f.player.clone(), node1, 1).unwrap());

        f.run(|| {
            assert_eq!(
                claim_quest_reward(&f.env, f.player.clone(), node1),
                Err(QuestError::InvalidStatus)
            );
        });
    }

    #[test]
    fn a_player_without_a_profile_cannot_claim() {
        let f = bare_fixture();
        let (chain_id, node1, _, _) = branching_chain(&f);

        f.run(|| start_chain(&f.env, f.player.clone(), chain_id).unwrap());
        f.run(|| record_progress(&f.env, f.player.clone(), node1, 3).unwrap());
        f.run(|| {
            assert_eq!(
                claim_quest_reward(&f.env, f.player.clone(), node1),
                Err(QuestError::ProfileNotFound)
            );
        });
    }

    // ── Branching narrative ───────────────────────────────────────────────

    #[test]
    fn choosing_a_branch_activates_the_successor_and_records_the_path() {
        let f = fixture();
        let (chain_id, node1, node2, _node3) = branching_chain(&f);
        f.run(|| start_chain(&f.env, f.player.clone(), chain_id).unwrap());
        f.run(|| record_progress(&f.env, f.player.clone(), node1, 3).unwrap());
        f.run(|| claim_quest_reward(&f.env, f.player.clone(), node1).unwrap());

        let next = f
            .run(|| choose_branch(&f.env, f.player.clone(), node1, 1).unwrap())
            .expect("choice 1 has a successor");
        assert_eq!(next.quest_id, node2);
        assert_eq!(next.status, QuestStatus::Active);
        assert_eq!(next.target, 2);

        f.run(|| {
            let progress = get_chain_progress(&f.env, &f.player, chain_id).unwrap();
            assert_eq!(progress.current_quest_id, node2);
            assert_eq!(progress.path.len(), 1);
            assert_eq!(progress.path.get(0).unwrap(), 1);
            assert_eq!(progress.steps_completed, 1);
            assert_eq!(progress.total_essence_earned, 100);
        });
    }

    #[test]
    fn the_other_branch_leads_somewhere_else() {
        let f = fixture();
        let (chain_id, node1, _node2, node3) = branching_chain(&f);
        f.run(|| start_chain(&f.env, f.player.clone(), chain_id).unwrap());
        f.run(|| record_progress(&f.env, f.player.clone(), node1, 3).unwrap());
        f.run(|| claim_quest_reward(&f.env, f.player.clone(), node1).unwrap());

        let next = f
            .run(|| choose_branch(&f.env, f.player.clone(), node1, 2).unwrap())
            .expect("choice 2 has a successor");
        assert_eq!(next.quest_id, node3);
        assert_eq!(next.target, 5);
    }

    #[test]
    fn a_terminus_branch_finishes_the_chain() {
        let f = fixture();
        let (chain_id, node1, node2, _) = branching_chain(&f);
        f.run(|| start_chain(&f.env, f.player.clone(), chain_id).unwrap());
        f.run(|| record_progress(&f.env, f.player.clone(), node1, 3).unwrap());
        f.run(|| claim_quest_reward(&f.env, f.player.clone(), node1).unwrap());
        f.run(|| choose_branch(&f.env, f.player.clone(), node1, 1).unwrap());

        f.run(|| record_progress(&f.env, f.player.clone(), node2, 2).unwrap());
        f.run(|| claim_quest_reward(&f.env, f.player.clone(), node2).unwrap());
        let end = f.run(|| choose_branch(&f.env, f.player.clone(), node2, 1).unwrap());

        assert!(end.is_none(), "a terminus branch has no successor");
        f.run(|| {
            let progress = get_chain_progress(&f.env, &f.player, chain_id).unwrap();
            assert!(progress.finished);
            assert_eq!(progress.steps_completed, 2);
            assert_eq!(progress.total_essence_earned, 350);
            assert_eq!(progress.path.len(), 2);
        });
    }

    #[test]
    fn a_branchless_node_finishes_the_chain_on_claim() {
        let f = fixture();
        let (chain_id, node1, _, node3) = branching_chain(&f);
        f.run(|| start_chain(&f.env, f.player.clone(), chain_id).unwrap());
        f.run(|| record_progress(&f.env, f.player.clone(), node1, 3).unwrap());
        f.run(|| claim_quest_reward(&f.env, f.player.clone(), node1).unwrap());
        f.run(|| choose_branch(&f.env, f.player.clone(), node1, 2).unwrap());

        f.run(|| record_progress(&f.env, f.player.clone(), node3, 5).unwrap());
        f.run(|| claim_quest_reward(&f.env, f.player.clone(), node3).unwrap());

        f.run(|| {
            let progress = get_chain_progress(&f.env, &f.player, chain_id).unwrap();
            assert!(progress.finished, "node3 has no branches, so the path ends");
            assert_eq!(progress.total_essence_earned, 500);
        });
    }

    #[test]
    fn branching_before_claiming_is_rejected() {
        let f = fixture();
        let (chain_id, node1, _, _) = branching_chain(&f);
        f.run(|| start_chain(&f.env, f.player.clone(), chain_id).unwrap());
        f.run(|| record_progress(&f.env, f.player.clone(), node1, 3).unwrap());

        f.run(|| {
            assert_eq!(
                choose_branch(&f.env, f.player.clone(), node1, 1),
                Err(QuestError::InvalidStatus)
            );
        });
    }

    #[test]
    fn a_branch_cannot_be_taken_twice() {
        let f = fixture();
        let (chain_id, node1, _, _) = branching_chain(&f);
        f.run(|| start_chain(&f.env, f.player.clone(), chain_id).unwrap());
        f.run(|| record_progress(&f.env, f.player.clone(), node1, 3).unwrap());
        f.run(|| claim_quest_reward(&f.env, f.player.clone(), node1).unwrap());
        f.run(|| choose_branch(&f.env, f.player.clone(), node1, 1).unwrap());

        f.run(|| {
            assert_eq!(
                choose_branch(&f.env, f.player.clone(), node1, 2),
                Err(QuestError::InvalidStatus),
                "the narrative path is immutable once walked"
            );
        });
    }

    #[test]
    fn an_unknown_choice_is_rejected() {
        let f = fixture();
        let (chain_id, node1, _, _) = branching_chain(&f);
        f.run(|| start_chain(&f.env, f.player.clone(), chain_id).unwrap());
        f.run(|| record_progress(&f.env, f.player.clone(), node1, 3).unwrap());
        f.run(|| claim_quest_reward(&f.env, f.player.clone(), node1).unwrap());

        f.run(|| {
            assert_eq!(
                choose_branch(&f.env, f.player.clone(), node1, 99),
                Err(QuestError::InvalidBranch)
            );
        });
    }

    // ── Mission bridge ────────────────────────────────────────────────────

    #[test]
    fn mission_completion_advances_matching_quests_only() {
        let f = fixture();
        let (chain_id, node1, _, _) = branching_chain(&f);
        f.run(|| start_chain(&f.env, f.player.clone(), chain_id).unwrap());

        // node1's objective is `scan`; a `trade` mission must not advance it.
        f.run(|| {
            assert_eq!(
                on_mission_completed(&f.env, &f.player, symbol_short!("trade"), 1),
                0
            );
            assert_eq!(
                get_quest_state(&f.env, &f.player, node1).unwrap().progress,
                0
            );
        });

        f.run(|| {
            assert_eq!(
                on_mission_completed(&f.env, &f.player, symbol_short!("scan"), 2),
                1
            );
            assert_eq!(
                get_quest_state(&f.env, &f.player, node1).unwrap().progress,
                2
            );
        });
    }

    #[test]
    fn mission_completion_can_finish_a_quest() {
        let f = fixture();
        let (chain_id, node1, _, _) = branching_chain(&f);
        f.run(|| start_chain(&f.env, f.player.clone(), chain_id).unwrap());

        f.run(|| on_mission_completed(&f.env, &f.player, symbol_short!("scan"), 3));
        f.run(|| {
            assert_eq!(
                get_quest_state(&f.env, &f.player, node1).unwrap().status,
                QuestStatus::Completed
            );
        });
    }

    #[test]
    fn mission_completion_is_a_no_op_for_a_player_with_no_quests() {
        let f = fixture();
        f.run(|| {
            assert_eq!(
                on_mission_completed(&f.env, &f.player, symbol_short!("scan"), 5),
                0
            );
        });
    }

    // ── Queries ───────────────────────────────────────────────────────────

    #[test]
    fn active_quest_states_are_listed() {
        let f = fixture();
        let (chain_id, node1, _, _) = branching_chain(&f);
        f.run(|| start_chain(&f.env, f.player.clone(), chain_id).unwrap());

        f.run(|| {
            let states = get_active_quest_states(&f.env, &f.player);
            assert_eq!(states.len(), 1);
            assert_eq!(states.get(0).unwrap().quest_id, node1);
        });
    }

    #[test]
    fn queries_return_none_for_unknown_ids() {
        let f = fixture();
        f.run(|| {
            assert!(get_chain(&f.env, 1).is_none());
            assert!(get_quest_node(&f.env, 1).is_none());
            assert!(get_quest_state(&f.env, &f.player, 1).is_none());
            assert!(get_chain_progress(&f.env, &f.player, 1).is_none());
            assert_eq!(get_active_quests(&f.env, &f.player).len(), 0);
        });
    }
}
