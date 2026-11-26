use crate::notifications::push_service::{notify, NotificationPriority};
use soroban_sdk::{symbol_short, Address, Env};

/// Warn when a balance drops below `threshold`; an empty balance is urgent.
pub fn check_low_resources(env: &Env, player: Address, balance: u32, threshold: u32) {
    if balance < threshold {
        let priority = if balance == 0 {
            NotificationPriority::Urgent
        } else {
            NotificationPriority::High
        };
        notify(env, player, symbol_short!("low_res"), priority);
    }
}

pub fn notify_rare_discovery(env: &Env, player: Address) {
    notify(
        env,
        player,
        symbol_short!("rare_find"),
        NotificationPriority::Normal,
    );
}

pub fn notify_crafting_complete(env: &Env, player: Address) {
    notify(
        env,
        player,
        symbol_short!("craft_ok"),
        NotificationPriority::Low,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::notifications::push_service::get_inbox;
    use soroban_sdk::testutils::Address as _;
    use soroban_sdk::{contract, contractimpl};

    #[contract]
    struct Stub;
    #[contractimpl]
    impl Stub {}

    #[test]
    fn low_resource_priority_scales_with_severity() {
        let env = Env::default();
        let id = env.register(Stub, ());
        let (a, b, c) = (
            Address::generate(&env),
            Address::generate(&env),
            Address::generate(&env),
        );
        env.as_contract(&id, || {
            check_low_resources(&env, a.clone(), 50, 10);
            check_low_resources(&env, b.clone(), 5, 10);
            check_low_resources(&env, c.clone(), 0, 10);
            assert_eq!(get_inbox(&env, &a).len(), 0);
            assert_eq!(
                get_inbox(&env, &b).get(0).unwrap().priority,
                NotificationPriority::High
            );
            assert_eq!(
                get_inbox(&env, &c).get(0).unwrap().priority,
                NotificationPriority::Urgent
            );
        });
    }
}
