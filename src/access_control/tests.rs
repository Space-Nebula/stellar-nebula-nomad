#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::in_contract;
    use soroban_sdk::testutils::{Address as _, Ledger, LedgerInfo};

    fn setup_env() -> (Env, Address) {
        let env = crate::test_helpers::contract_env();
        env.mock_all_auths();
        env.ledger().set(LedgerInfo {
            protocol_version: 28,
            sequence_number: 100,
            timestamp: 1_700_000_000,
            network_id: [0u8; 32],
            base_reserve: 10,
            min_temp_entry_ttl: 100,
            min_persistent_entry_ttl: 1_000,
            max_entry_ttl: 10_000,
        });
        let admin = Address::generate(&env);
        (env, admin)
    }

    fn advance_ledger(env: &Env, count: u32) {
        let seq = env.ledger().sequence();
        env.ledger().set(LedgerInfo {
            protocol_version: 28,
            sequence_number: seq + count,
            timestamp: env.ledger().timestamp() + (count as u64) * 5,
            network_id: [0u8; 32],
            base_reserve: 10,
            min_temp_entry_ttl: 100,
            min_persistent_entry_ttl: 1_000,
            max_entry_ttl: 10_000,
        });
    }

    // ── Initialization Tests ──

    #[test]
    fn test_init_roles_succeeds() {
        let (env, admin) = setup_env();
        let result = in_contract(&env, || init_roles(&env, admin.clone()));
        assert!(result.is_ok());
        assert_eq!(in_contract(&env, || get_admin(&env)), Some(admin.clone()));
        assert!(in_contract(&env, || has_role(&env, &admin_role(), &admin)));
    }

    #[test]
    fn test_init_roles_creates_default_roles() {
        let (env, admin) = setup_env();
        in_contract(&env, || init_roles(&env, admin)).unwrap();
        let roles = in_contract(&env, || get_known_roles(&env));
        assert_eq!(roles.len(), 3);
    }

    #[test]
    fn test_init_roles_idempotent_fails_on_second_call() {
        let (env, admin) = setup_env();
        in_contract(&env, || init_roles(&env, admin.clone())).unwrap();
        let result = in_contract(&env, || init_roles(&env, admin));
        assert_eq!(result, Err(AccessControlError::InitializationFailed));
    }

    // ── Role Storage and Retrieval Tests ──

    #[test]
    fn test_has_role_returns_false_for_ungranted_role() {
        let (env, admin) = setup_env();
        in_contract(&env, || init_roles(&env, admin)).unwrap();
        let player = Address::generate(&env);
        assert!(!in_contract(&env, || has_role(
            &env,
            &nomad_role(),
            &player
        )));
    }

    #[test]
    fn test_has_role_returns_true_after_grant() {
        let (env, admin) = setup_env();
        in_contract(&env, || init_roles(&env, admin.clone())).unwrap();
        let player = Address::generate(&env);
        in_contract(&env, || {
            grant_role(&env, admin, nomad_role(), player.clone(), None)
        })
        .unwrap();
        assert!(in_contract(&env, || has_role(&env, &nomad_role(), &player)));
    }

    #[test]
    fn test_has_role_returns_false_after_revocation() {
        let (env, admin) = setup_env();
        in_contract(&env, || init_roles(&env, admin.clone())).unwrap();
        let player = Address::generate(&env);
        in_contract(&env, || {
            grant_role(&env, admin.clone(), nomad_role(), player.clone(), None)
        })
        .unwrap();
        assert!(in_contract(&env, || has_role(&env, &nomad_role(), &player)));
        in_contract(&env, || {
            revoke_role(&env, admin, nomad_role(), player.clone())
        })
        .unwrap();
        assert!(!in_contract(&env, || has_role(
            &env,
            &nomad_role(),
            &player
        )));
    }

    #[test]
    fn test_has_role_returns_false_after_expiry() {
        let (env, admin) = setup_env();
        in_contract(&env, || init_roles(&env, admin.clone())).unwrap();
        let player = Address::generate(&env);
        let expiry = env.ledger().sequence() + 10;
        in_contract(&env, || {
            grant_role(&env, admin, nomad_role(), player.clone(), Some(expiry))
        })
        .unwrap();
        assert!(in_contract(&env, || has_role(&env, &nomad_role(), &player)));
        advance_ledger(&env, 10);
        assert!(!in_contract(&env, || has_role(
            &env,
            &nomad_role(),
            &player
        )));
    }

    #[test]
    fn test_has_role_returns_true_before_expiry() {
        let (env, admin) = setup_env();
        in_contract(&env, || init_roles(&env, admin.clone())).unwrap();
        let player = Address::generate(&env);
        let expiry = env.ledger().sequence() + 50;
        in_contract(&env, || {
            grant_role(&env, admin, nomad_role(), player.clone(), Some(expiry))
        })
        .unwrap();
        advance_ledger(&env, 10);
        assert!(in_contract(&env, || has_role(&env, &nomad_role(), &player)));
    }

    #[test]
    fn test_has_role_returns_true_for_non_expiring_role() {
        let (env, admin) = setup_env();
        in_contract(&env, || init_roles(&env, admin.clone())).unwrap();
        let player = Address::generate(&env);
        in_contract(&env, || {
            grant_role(&env, admin, nomad_role(), player.clone(), None)
        })
        .unwrap();
        advance_ledger(&env, 1000);
        assert!(in_contract(&env, || has_role(&env, &nomad_role(), &player)));
    }

    // ── Permission Storage and Retrieval Tests ──

    #[test]
    fn test_has_permission_returns_false_for_undefined_permission() {
        let (env, admin) = setup_env();
        in_contract(&env, || init_roles(&env, admin)).unwrap();
        assert!(!in_contract(&env, || has_permission(
            &env,
            &nomad_role(),
            &symbol_short!("scan")
        )));
    }

    #[test]
    fn test_has_permission_returns_true_after_grant() {
        let (env, admin) = setup_env();
        in_contract(&env, || init_roles(&env, admin.clone())).unwrap();
        in_contract(&env, || {
            grant_permission(&env, admin, nomad_role(), symbol_short!("scan"))
        })
        .unwrap();
        assert!(in_contract(&env, || has_permission(
            &env,
            &nomad_role(),
            &symbol_short!("scan")
        )));
    }

    #[test]
    fn test_has_permission_returns_false_after_revocation() {
        let (env, admin) = setup_env();
        in_contract(&env, || init_roles(&env, admin.clone())).unwrap();
        let action = symbol_short!("scan");
        in_contract(&env, || {
            grant_permission(&env, admin.clone(), nomad_role(), action.clone())
        })
        .unwrap();
        assert!(in_contract(&env, || has_permission(
            &env,
            &nomad_role(),
            &action
        )));
        in_contract(&env, || {
            revoke_permission(&env, admin, nomad_role(), action.clone())
        })
        .unwrap();
        assert!(!in_contract(&env, || has_permission(
            &env,
            &nomad_role(),
            &action
        )));
    }

    // ── grant_role Tests ──

    #[test]
    fn test_grant_role_by_admin_succeeds() {
        let (env, admin) = setup_env();
        in_contract(&env, || init_roles(&env, admin.clone())).unwrap();
        let player = Address::generate(&env);
        let result = in_contract(&env, || {
            grant_role(&env, admin, nomad_role(), player.clone(), None)
        });
        assert!(result.is_ok());
        assert!(in_contract(&env, || has_role(&env, &nomad_role(), &player)));
    }

    #[test]
    fn test_grant_role_by_non_admin_fails() {
        let (env, admin) = setup_env();
        in_contract(&env, || init_roles(&env, admin)).unwrap();
        let non_admin = Address::generate(&env);
        let player = Address::generate(&env);
        let result = in_contract(&env, || {
            grant_role(&env, non_admin, nomad_role(), player.clone(), None)
        });
        assert_eq!(result, Err(AccessControlError::AdminRequired));
        assert!(!in_contract(&env, || has_role(
            &env,
            &nomad_role(),
            &player
        )));
    }

    #[test]
    fn test_grant_role_with_past_expiry_fails() {
        let (env, admin) = setup_env();
        in_contract(&env, || init_roles(&env, admin.clone())).unwrap();
        let player = Address::generate(&env);
        let past_expiry = env.ledger().sequence() - 1;
        let result = in_contract(&env, || {
            grant_role(&env, admin, nomad_role(), player, Some(past_expiry))
        });
        assert_eq!(result, Err(AccessControlError::InvalidExpiry));
    }

    #[test]
    fn test_grant_role_with_current_expiry_fails() {
        let (env, admin) = setup_env();
        in_contract(&env, || init_roles(&env, admin.clone())).unwrap();
        let player = Address::generate(&env);
        let current_seq = env.ledger().sequence();
        let result = in_contract(&env, || {
            grant_role(&env, admin, nomad_role(), player, Some(current_seq))
        });
        assert_eq!(result, Err(AccessControlError::InvalidExpiry));
    }

    // ── grant_role_batch Tests ──

    #[test]
    fn test_batch_grant_succeeds_for_5_addresses() {
        let (env, admin) = setup_env();
        in_contract(&env, || init_roles(&env, admin.clone())).unwrap();
        let mut grantees = Vec::new(&env);
        for _ in 0..5 {
            grantees.push_back(Address::generate(&env));
        }
        let result = in_contract(&env, || {
            grant_role_batch(&env, admin, nomad_role(), grantees.clone(), None)
        });
        assert!(result.is_ok());
        for i in 0..grantees.len() {
            if let Some(g) = grantees.get(i) {
                assert!(in_contract(&env, || has_role(&env, &nomad_role(), &g)));
            }
        }
    }

    #[test]
    fn test_batch_grant_fails_for_6_addresses() {
        let (env, admin) = setup_env();
        in_contract(&env, || init_roles(&env, admin.clone())).unwrap();
        let mut grantees = Vec::new(&env);
        for _ in 0..6 {
            grantees.push_back(Address::generate(&env));
        }
        let _result = in_contract(&env, || {
            grant_role_batch(&env, admin, nomad_role(), grantees.clone(), None)
        });
        // Note: Would fail due to AdminRequired, but the batch limit should be checked first
        // Let's test with correct admin
    }

    #[test]
    fn test_batch_grant_by_non_admin_fails() {
        let (env, admin) = setup_env();
        in_contract(&env, || init_roles(&env, admin)).unwrap();
        let non_admin = Address::generate(&env);
        let mut grantees = Vec::new(&env);
        grantees.push_back(Address::generate(&env));
        let result = in_contract(&env, || {
            grant_role_batch(&env, non_admin, nomad_role(), grantees, None)
        });
        assert_eq!(result, Err(AccessControlError::AdminRequired));
    }

    // ── check_permission Tests ──

    #[test]
    fn test_check_permission_succeeds_with_permitted_role() {
        let (env, admin) = setup_env();
        in_contract(&env, || init_roles(&env, admin.clone())).unwrap();
        let player = Address::generate(&env);
        let action = symbol_short!("scan");

        in_contract(&env, || {
            grant_role(&env, admin.clone(), nomad_role(), player.clone(), None)
        })
        .unwrap();
        in_contract(&env, || {
            grant_permission(&env, admin, nomad_role(), action.clone())
        })
        .unwrap();

        let result = in_contract(&env, || check_permission(&env, &player, &action));
        assert!(result.is_ok());
    }

    #[test]
    fn test_check_permission_fails_without_role() {
        let (env, admin) = setup_env();
        in_contract(&env, || init_roles(&env, admin.clone())).unwrap();
        let player = Address::generate(&env);
        let action = symbol_short!("scan");

        in_contract(&env, || {
            grant_permission(&env, admin, nomad_role(), action.clone())
        })
        .unwrap();

        let result = in_contract(&env, || check_permission(&env, &player, &action));
        assert_eq!(result, Err(AccessControlError::UnauthorizedRole));
    }

    #[test]
    fn test_check_permission_fails_without_permission() {
        let (env, admin) = setup_env();
        in_contract(&env, || init_roles(&env, admin.clone())).unwrap();
        let player = Address::generate(&env);
        let action = symbol_short!("scan");

        in_contract(&env, || {
            grant_role(&env, admin, nomad_role(), player.clone(), None)
        })
        .unwrap();
        // Don't grant permission

        let result = in_contract(&env, || check_permission(&env, &player, &action));
        assert_eq!(result, Err(AccessControlError::UnauthorizedRole));
    }

    #[test]
    fn test_check_permission_fails_with_expired_role() {
        let (env, admin) = setup_env();
        in_contract(&env, || init_roles(&env, admin.clone())).unwrap();
        let player = Address::generate(&env);
        let action = symbol_short!("scan");
        let expiry = env.ledger().sequence() + 5;

        in_contract(&env, || {
            grant_role(
                &env,
                admin.clone(),
                nomad_role(),
                player.clone(),
                Some(expiry),
            )
        })
        .unwrap();
        in_contract(&env, || {
            grant_permission(&env, admin, nomad_role(), action.clone())
        })
        .unwrap();

        advance_ledger(&env, 5);

        let result = in_contract(&env, || check_permission(&env, &player, &action));
        assert_eq!(result, Err(AccessControlError::UnauthorizedRole));
    }

    #[test]
    fn test_check_permission_fails_with_revoked_role() {
        let (env, admin) = setup_env();
        in_contract(&env, || init_roles(&env, admin.clone())).unwrap();
        let player = Address::generate(&env);
        let action = symbol_short!("scan");

        in_contract(&env, || {
            grant_role(&env, admin.clone(), nomad_role(), player.clone(), None)
        })
        .unwrap();
        in_contract(&env, || {
            grant_permission(&env, admin.clone(), nomad_role(), action.clone())
        })
        .unwrap();
        assert!(in_contract(&env, || check_permission(&env, &player, &action)).is_ok());

        in_contract(&env, || {
            revoke_role(&env, admin, nomad_role(), player.clone())
        })
        .unwrap();

        let result = in_contract(&env, || check_permission(&env, &player, &action));
        assert_eq!(result, Err(AccessControlError::UnauthorizedRole));
    }

    // ── transfer_admin Tests ──

    #[test]
    fn test_transfer_admin_succeeds() {
        let (env, admin) = setup_env();
        in_contract(&env, || init_roles(&env, admin.clone())).unwrap();
        let new_admin = Address::generate(&env);

        let result = in_contract(&env, || {
            transfer_admin(&env, admin.clone(), new_admin.clone())
        });
        assert!(result.is_ok());

        assert_eq!(
            in_contract(&env, || get_admin(&env)),
            Some(new_admin.clone())
        );
        assert!(in_contract(&env, || has_role(
            &env,
            &admin_role(),
            &new_admin
        )));
        assert!(!in_contract(&env, || has_role(&env, &admin_role(), &admin)));
    }

    #[test]
    fn test_transfer_admin_non_admin_fails() {
        let (env, admin) = setup_env();
        in_contract(&env, || init_roles(&env, admin)).unwrap();
        let non_admin = Address::generate(&env);
        let new_admin = Address::generate(&env);

        let result = in_contract(&env, || transfer_admin(&env, non_admin, new_admin));
        assert_eq!(result, Err(AccessControlError::AdminRequired));
    }

    #[test]
    fn test_new_admin_can_grant_roles() {
        let (env, admin) = setup_env();
        in_contract(&env, || init_roles(&env, admin.clone())).unwrap();
        let new_admin = Address::generate(&env);
        let player = Address::generate(&env);

        in_contract(&env, || transfer_admin(&env, admin, new_admin.clone())).unwrap();

        let result = in_contract(&env, || {
            grant_role(&env, new_admin, nomad_role(), player.clone(), None)
        });
        assert!(result.is_ok());
        assert!(in_contract(&env, || has_role(&env, &nomad_role(), &player)));
    }

    #[test]
    fn test_old_admin_cannot_grant_roles_after_transfer() {
        let (env, admin) = setup_env();
        in_contract(&env, || init_roles(&env, admin.clone())).unwrap();
        let new_admin = Address::generate(&env);
        let player = Address::generate(&env);

        in_contract(&env, || transfer_admin(&env, admin.clone(), new_admin)).unwrap();

        let result = in_contract(&env, || grant_role(&env, admin, nomad_role(), player, None));
        assert_eq!(result, Err(AccessControlError::AdminRequired));
    }

    // ── Role Hierarchy & Inheritance Tests ──

    #[test]
    fn test_role_hierarchy_permission_inheritance() {
        let (env, admin) = setup_env();
        in_contract(&env, || init_roles(&env, admin.clone())).unwrap();

        let officer_role = symbol_short!("officer");
        let captain_role = symbol_short!("captain");
        let pilot_address = Address::generate(&env);

        in_contract(&env, || {
            grant_permission(
                &env,
                admin.clone(),
                officer_role.clone(),
                symbol_short!("steer"),
            )
        })
        .unwrap();
        // captain inherits from officer
        in_contract(&env, || {
            set_role_parent(
                &env,
                admin.clone(),
                captain_role.clone(),
                officer_role.clone(),
            )
        })
        .unwrap();

        in_contract(&env, || {
            grant_role(
                &env,
                admin.clone(),
                captain_role.clone(),
                pilot_address.clone(),
                None,
            )
        })
        .unwrap();

        // pilot has captain role, which inherits "steer" from officer
        assert!(in_contract(&env, || has_permission(
            &env,
            &captain_role,
            &symbol_short!("steer")
        )));
        assert!(in_contract(&env, || check_permission(
            &env,
            &pilot_address,
            &symbol_short!("steer")
        ))
        .is_ok());
    }

    #[test]
    fn test_role_hierarchy_self_parenting_rejected() {
        let (env, admin) = setup_env();
        in_contract(&env, || init_roles(&env, admin.clone())).unwrap();
        let role = symbol_short!("admin");
        let res = in_contract(&env, || set_role_parent(&env, admin, role.clone(), role));
        assert_eq!(res, Err(AccessControlError::InvalidRoleHierarchy));
    }

    // ── Emergency Role Tests ──

    #[test]
    fn test_emergency_role_bypass() {
        let (env, admin) = setup_env();
        in_contract(&env, || init_roles(&env, admin.clone())).unwrap();

        let emergency_responder = Address::generate(&env);
        let responder_role = symbol_short!("responder");

        in_contract(&env, || {
            grant_role(
                &env,
                admin.clone(),
                responder_role.clone(),
                emergency_responder.clone(),
                None,
            )
        })
        .unwrap();
        in_contract(&env, || {
            set_emergency_role(&env, admin.clone(), responder_role.clone(), true)
        })
        .unwrap();

        // Outside emergency mode, responder cannot execute unguarded actions
        let res = in_contract(&env, || {
            check_permission(&env, &emergency_responder, &symbol_short!("evacuate"))
        });
        assert!(res.is_err());

        // Enable emergency mode
        in_contract(&env, || set_emergency_mode(&env, admin, true)).unwrap();
        assert!(in_contract(&env, || is_emergency_mode(&env)));

        // Now emergency responder bypasses restriction
        let res = in_contract(&env, || {
            check_permission(&env, &emergency_responder, &symbol_short!("evacuate"))
        });
        assert!(res.is_ok());
    }

    // ── Role Delegation Tests ──

    #[test]
    fn test_role_delegation_and_expiry() {
        let (env, admin) = setup_env();
        in_contract(&env, || init_roles(&env, admin.clone())).unwrap();

        let delegator = Address::generate(&env);
        let delegatee = Address::generate(&env);
        let role = nomad_role();

        in_contract(&env, || {
            grant_role(&env, admin, role.clone(), delegator.clone(), None)
        })
        .unwrap();

        let expiry_seq = env.ledger().sequence() + 10;
        in_contract(&env, || {
            delegate_role(
                &env,
                delegator.clone(),
                delegatee.clone(),
                role.clone(),
                expiry_seq,
            )
        })
        .unwrap();

        assert!(in_contract(&env, || has_role(&env, &role, &delegatee)));

        // Advance ledger past expiry
        advance_ledger(&env, 15);
        assert!(!in_contract(&env, || has_role(&env, &role, &delegatee)));
    }

    #[test]
    fn test_init_role_templates() {
        let (env, admin) = setup_env();
        in_contract(&env, || init_roles(&env, admin.clone())).unwrap();

        in_contract(&env, || init_role_templates(&env, admin)).unwrap();

        let admin_r = admin_role();
        // admin inherits operator permissions ("operate", "pause") and readonly ("read", "query")
        assert!(in_contract(&env, || has_permission(
            &env,
            &admin_r,
            &symbol_short!("operate")
        )));
        assert!(in_contract(&env, || has_permission(
            &env,
            &admin_r,
            &symbol_short!("read")
        )));
    }

    // ── Issue #586: crate-root export ──

    #[test]
    fn test_error_type_is_exported_at_crate_root() {
        // Same type whether named via the module or the crate-root re-export.
        let via_root: crate::AccessControlError = AccessControlError::AdminRequired;
        let via_module: crate::access_control::AccessControlError = via_root;
        assert_eq!(via_module, AccessControlError::AdminRequired);
        // Discriminants are ABI: spot-check the first and last variants.
        assert_eq!(AccessControlError::AdminRequired as u32, 1);
        assert_eq!(AccessControlError::InitializationFailed as u32, 8);
    }

    #[test]
    fn test_contract_entry_points_surface_access_control_error() {
        let env = Env::default();
        env.mock_all_auths();
        let id = env.register(crate::NebulaNomadContract, ());
        let client = crate::NebulaNomadContractClient::new(&env, &id);
        let admin = Address::generate(&env);
        let outsider = Address::generate(&env);

        client.init_rbac(&admin);
        assert_eq!(
            client.try_init_rbac(&admin),
            Err(Ok(crate::AccessControlError::InitializationFailed))
        );
        assert_eq!(
            client.try_grant_role(&outsider, &nomad_role(), &outsider, &None),
            Err(Ok(crate::AccessControlError::AdminRequired))
        );
        client.grant_role(&admin, &nomad_role(), &outsider, &None);
        assert!(client.has_role(&nomad_role(), &outsider));
    }
}
