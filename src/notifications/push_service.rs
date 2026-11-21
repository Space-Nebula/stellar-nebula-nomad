//! Player notifications (#710): priorities, per-player preferences,
//! multi-channel delivery and aggregation of repeats.
//!
//! The contract cannot reach phones or mailboxes itself. Each enabled channel
//! is a separate event that an off-chain relayer delivers. The in-app channel
//! keeps the original `("notify", player) -> message` event shape.

use soroban_sdk::{contracterror, contracttype, symbol_short, Address, Env, Symbol, Vec};

/// In-app inbox / legacy `notify` event.
pub const CHANNEL_IN_APP: u32 = 1;
/// Mobile push relayer.
pub const CHANNEL_PUSH: u32 = 2;
/// Email relayer.
pub const CHANNEL_EMAIL: u32 = 4;
pub const ALL_CHANNELS: u32 = CHANNEL_IN_APP | CHANNEL_PUSH | CHANNEL_EMAIL;
/// Inbox entries kept per player (oldest dropped first).
pub const MAX_INBOX: u32 = 20;
/// Message types a player may mute.
pub const MAX_MUTED: u32 = 16;
/// Repeats of the same message within this window fold into one entry.
pub const NOTIFICATION_AGGREGATION_SECS: u64 = 3_600;

#[contracttype]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum NotificationPriority {
    Low = 0,
    Normal = 1,
    High = 2,
    /// Always delivered in-app, even when muted or below `min_priority`.
    Urgent = 3,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct Notification {
    pub user: Address,
    pub message: Symbol,
    /// Ledger time of the latest occurrence.
    pub timestamp: u64,
    pub priority: NotificationPriority,
    /// Occurrences folded into this entry.
    pub count: u32,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct NotificationPreferences {
    pub min_priority: NotificationPriority,
    /// Bitmask of `CHANNEL_*`.
    pub channels: u32,
    pub muted: Vec<Symbol>,
}

#[contracttype]
#[derive(Clone)]
pub enum NotificationKey {
    NotifyPrefs(Address),
    NotifyInbox(Address),
}

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum NotificationError {
    /// Channel mask has bits outside `ALL_CHANNELS`.
    InvalidChannels = 1,
    /// More than `MAX_MUTED` muted message types.
    TooManyMuted = 2,
}

impl crate::error_standard::StandardContractError for NotificationError {
    fn descriptor(self) -> crate::error_standard::ErrorDescriptor {
        crate::error_standard::ErrorDescriptor {
            module: "push_service",
            code: self as u32,
            kind: crate::error_standard::ErrorKind::Validation,
            retryable: false,
        }
    }
}

/// Defaults reproduce the original behaviour: everything, in-app only.
pub fn get_notification_preferences(env: &Env, player: &Address) -> NotificationPreferences {
    env.storage()
        .persistent()
        .get(&NotificationKey::NotifyPrefs(player.clone()))
        .unwrap_or_else(|| NotificationPreferences {
            min_priority: NotificationPriority::Low,
            channels: CHANNEL_IN_APP,
            muted: Vec::new(env),
        })
}

pub fn set_notification_preferences(
    env: &Env,
    player: Address,
    prefs: NotificationPreferences,
) -> Result<(), NotificationError> {
    player.require_auth();
    if prefs.channels & !ALL_CHANNELS != 0 {
        return Err(NotificationError::InvalidChannels);
    }
    if prefs.muted.len() > MAX_MUTED {
        return Err(NotificationError::TooManyMuted);
    }
    env.storage()
        .persistent()
        .set(&NotificationKey::NotifyPrefs(player), &prefs);
    Ok(())
}

/// Latest notifications for a player, oldest first.
pub fn get_inbox(env: &Env, player: &Address) -> Vec<Notification> {
    env.storage()
        .temporary()
        .get(&NotificationKey::NotifyInbox(player.clone()))
        .unwrap_or_else(|| Vec::new(env))
}

pub fn clear_inbox(env: &Env, player: Address) {
    player.require_auth();
    env.storage()
        .temporary()
        .remove(&NotificationKey::NotifyInbox(player));
}

/// Deliver `message` to `player` according to their preferences.
///
/// Returns the stored inbox entry, or `None` when the player filtered it out.
/// A repeat within `NOTIFICATION_AGGREGATION_SECS` bumps the existing entry's
/// `count` and emits nothing, so bursts do not spam every channel.
pub fn notify(
    env: &Env,
    player: Address,
    message: Symbol,
    priority: NotificationPriority,
) -> Option<Notification> {
    let prefs = get_notification_preferences(env, &player);
    let urgent = priority == NotificationPriority::Urgent;
    if !urgent && (priority < prefs.min_priority || prefs.muted.contains(&message)) {
        return None;
    }

    let now = env.ledger().timestamp();
    // Inbox is temporary storage: if it expires, notifications simply restart.
    let key = NotificationKey::NotifyInbox(player.clone());
    let mut inbox = get_inbox(env, &player);

    for i in 0..inbox.len() {
        let mut entry = inbox.get(i).unwrap();
        if entry.message == message
            && now.saturating_sub(entry.timestamp) <= NOTIFICATION_AGGREGATION_SECS
        {
            entry.count = entry.count.saturating_add(1);
            entry.timestamp = now;
            entry.priority = entry.priority.max(priority);
            inbox.set(i, entry.clone());
            env.storage().temporary().set(&key, &inbox);
            return Some(entry);
        }
    }

    let entry = Notification {
        user: player.clone(),
        message: message.clone(),
        timestamp: now,
        priority,
        count: 1,
    };
    if inbox.len() >= MAX_INBOX {
        inbox.remove(0);
    }
    inbox.push_back(entry.clone());
    env.storage().temporary().set(&key, &inbox);

    let channels = if urgent {
        prefs.channels | CHANNEL_IN_APP
    } else {
        prefs.channels
    };
    if channels & CHANNEL_IN_APP != 0 {
        env.events()
            .publish((symbol_short!("notify"), player.clone()), message.clone());
    }
    if channels & CHANNEL_PUSH != 0 {
        env.events().publish(
            (
                symbol_short!("notify"),
                symbol_short!("push"),
                player.clone(),
            ),
            (message.clone(), priority),
        );
    }
    if channels & CHANNEL_EMAIL != 0 {
        env.events().publish(
            (symbol_short!("notify"), symbol_short!("email"), player),
            (message, priority),
        );
    }
    Some(entry)
}

/// Legacy entry point: a `Normal` priority notification.
pub fn emit_notification(env: &Env, player: Address, message: Symbol) {
    notify(env, player, message, NotificationPriority::Normal);
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::testutils::{Address as _, Events as _, Ledger as _};
    use soroban_sdk::{contract, contractimpl};

    #[contract]
    struct Stub;
    #[contractimpl]
    impl Stub {}

    fn setup() -> (Env, Address, Address) {
        let env = Env::default();
        env.mock_all_auths();
        let id = env.register(Stub, ());
        let player = Address::generate(&env);
        (env, id, player)
    }

    fn event_count(env: &Env) -> usize {
        env.events().all().events().len()
    }

    #[test]
    fn default_delivers_in_app_only() {
        let (env, id, player) = setup();
        env.as_contract(&id, || {
            emit_notification(&env, player.clone(), symbol_short!("rare_find"));
            assert_eq!(event_count(&env), 1);
            let inbox = get_inbox(&env, &player);
            assert_eq!(inbox.len(), 1);
            assert_eq!(inbox.get(0).unwrap().priority, NotificationPriority::Normal);
        });
    }

    #[test]
    fn repeats_aggregate_without_new_events() {
        let (env, id, player) = setup();
        env.as_contract(&id, || {
            let n = || {
                notify(
                    &env,
                    player.clone(),
                    symbol_short!("low_res"),
                    NotificationPriority::Low,
                )
            };
            n();
            env.ledger().set_timestamp(100);
            let entry = n().unwrap();
            assert_eq!((entry.count, entry.timestamp), (2, 100));
            assert_eq!(event_count(&env), 1); // only the first call emitted
            assert_eq!(get_inbox(&env, &player).len(), 1);

            env.ledger()
                .set_timestamp(100 + NOTIFICATION_AGGREGATION_SECS + 1);
            assert_eq!(n().unwrap().count, 1);
            assert_eq!(get_inbox(&env, &player).len(), 2);
        });
    }

    #[test]
    fn preferences_filter_and_route_channels() {
        let (env, id, player) = setup();
        let mut muted = Vec::new(&env);
        muted.push_back(symbol_short!("craft_ok"));
        let prefs = NotificationPreferences {
            min_priority: NotificationPriority::High,
            channels: CHANNEL_PUSH | CHANNEL_EMAIL,
            muted,
        };
        env.as_contract(&id, || {
            set_notification_preferences(&env, player.clone(), prefs.clone()).unwrap();
        });
        env.as_contract(&id, || {
            assert_eq!(get_notification_preferences(&env, &player), prefs);
            // Below min priority and muted messages are dropped.
            assert!(notify(
                &env,
                player.clone(),
                symbol_short!("rare"),
                NotificationPriority::Normal
            )
            .is_none());
            assert!(notify(
                &env,
                player.clone(),
                symbol_short!("craft_ok"),
                NotificationPriority::High
            )
            .is_none());
            // High goes to push and email, not in-app.
            notify(
                &env,
                player.clone(),
                symbol_short!("raid"),
                NotificationPriority::High,
            )
            .unwrap();
            assert_eq!(event_count(&env), 2);
        });
        env.as_contract(&id, || {
            // Urgent bypasses mute and always adds in-app.
            notify(
                &env,
                player.clone(),
                symbol_short!("craft_ok"),
                NotificationPriority::Urgent,
            )
            .unwrap();
            assert_eq!(event_count(&env), 3);
        });
    }

    #[test]
    fn invalid_preferences_are_rejected() {
        let (env, id, player) = setup();
        env.as_contract(&id, || {
            let bad = NotificationPreferences {
                min_priority: NotificationPriority::Low,
                channels: 8,
                muted: Vec::new(&env),
            };
            assert_eq!(
                set_notification_preferences(&env, player.clone(), bad),
                Err(NotificationError::InvalidChannels)
            );
        });
        env.as_contract(&id, || {
            let mut muted = Vec::new(&env);
            for _ in 0..=MAX_MUTED {
                muted.push_back(symbol_short!("x"));
            }
            let bad = NotificationPreferences {
                min_priority: NotificationPriority::Low,
                channels: CHANNEL_IN_APP,
                muted,
            };
            assert_eq!(
                set_notification_preferences(&env, player.clone(), bad),
                Err(NotificationError::TooManyMuted)
            );
        });
    }

    #[test]
    fn inbox_is_capped_and_clearable() {
        let (env, id, player) = setup();
        env.as_contract(&id, || {
            for i in 0..(MAX_INBOX + 3) {
                env.ledger()
                    .set_timestamp(u64::from(i) * (NOTIFICATION_AGGREGATION_SECS + 1));
                notify(
                    &env,
                    player.clone(),
                    symbol_short!("tick"),
                    NotificationPriority::Low,
                );
            }
            assert_eq!(get_inbox(&env, &player).len(), MAX_INBOX);
        });
        env.as_contract(&id, || clear_inbox(&env, player.clone()));
        env.as_contract(&id, || assert_eq!(get_inbox(&env, &player).len(), 0));
    }
}
