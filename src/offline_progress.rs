use soroban_sdk::{
    contract, contractimpl, contracttype, contracterror, symbol_short,
    Address, Env, Vec,
};

/// Offline progress tracking record
#[derive(Clone)]
#[contracttype]
pub struct OfflineProgress {
    pub player: Address,
    pub last_active: u64,
    pub total_accrued: i128,
}

/// Offline yield claim result
#[derive(Clone)]
#[contracttype]
pub struct OfflineYieldClaim {
    pub player: Address,
    pub offline_duration: u64,
    pub yield_amount: i128,
    pub claimed_at: u64,
}

#[derive(Clone)]
#[contracttype]
pub enum DataKey {
    Progress(Address),
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum OfflineError {
    NoAccrualAvailable = 1,
    NotInitialized = 2,
}

const MAX_ACCRUAL_HOURS: u64 = 48;
const SECONDS_PER_HOUR: u64 = 3600;
const BASE_YIELD_PER_HOUR: i128 = 100;

#[contract]
pub struct OfflineProgressTracker;

#[contractimpl]
impl OfflineProgressTracker {
    /// Record player's last active timestamp
    pub fn record_last_active(env: Env, player: Address) {
        player.require_auth();

        let now = env.ledger().timestamp();
        let progress = OfflineProgress {
            player: player.clone(),
            last_active: now,
            total_accrued: 0,
        };

        env.storage()
            .persistent()
            .set(&DataKey::Progress(player), &progress);
    }

    /// Calculate and claim offline yield
    pub fn claim_offline_yield(
        env: Env,
        player: Address,
    ) -> Result<OfflineYieldClaim, OfflineError> {
        player.require_auth();

        let progress: OfflineProgress = env
            .storage()
            .persistent()
            .get(&DataKey::Progress(player.clone()))
            .ok_or(OfflineError::NotInitialized)?;

        let now = env.ledger().timestamp();
        let offline_duration = now.saturating_sub(progress.last_active);

        if offline_duration == 0 {
            return Err(OfflineError::NoAccrualAvailable);
        }

        let max_accrual_seconds = MAX_ACCRUAL_HOURS * SECONDS_PER_HOUR;
        let capped_duration = offline_duration.min(max_accrual_seconds);
        let hours_offline = capped_duration / SECONDS_PER_HOUR;
        let yield_amount = (hours_offline as i128) * BASE_YIELD_PER_HOUR;

        let updated_progress = OfflineProgress {
            player: player.clone(),
            last_active: now,
            total_accrued: progress.total_accrued + yield_amount,
        };

        env.storage()
            .persistent()
            .set(&DataKey::Progress(player.clone()), &updated_progress);

        let claim = OfflineYieldClaim {
            player: player.clone(),
            offline_duration: capped_duration,
            yield_amount,
            claimed_at: now,
        };

        env.events().publish(
            (symbol_short!("offline"), player),
            (offline_duration, yield_amount, now),
        );

        Ok(claim)
    }

    /// Get player's offline progress status
    pub fn get_offline_progress(env: Env, player: Address) -> Option<OfflineProgress> {
        env.storage()
            .persistent()
            .get(&DataKey::Progress(player))
    }

    /// Calculate pending offline yield without claiming
    pub fn calculate_pending_yield(env: Env, player: Address) -> Result<i128, OfflineError> {
        let progress: OfflineProgress = env
            .storage()
            .persistent()
            .get(&DataKey::Progress(player))
            .ok_or(OfflineError::NotInitialized)?;

        let now = env.ledger().timestamp();
        let offline_duration = now.saturating_sub(progress.last_active);
        let max_accrual_seconds = MAX_ACCRUAL_HOURS * SECONDS_PER_HOUR;
        let capped_duration = offline_duration.min(max_accrual_seconds);
        let hours_offline = capped_duration / SECONDS_PER_HOUR;

        Ok((hours_offline as i128) * BASE_YIELD_PER_HOUR)
    }

    /// Batch claim for multiple players (admin utility)
    pub fn batch_claim_offline_yield(
        env: Env,
        players: Vec<Address>,
    ) -> Vec<OfflineYieldClaim> {
        let mut claims = Vec::new(&env);

        for i in 0..players.len() {
            let player = players.get(i).unwrap();
            if let Ok(claim) = Self::claim_offline_yield(env.clone(), player) {
                claims.push_back(claim);
            }
        }

        claims
    }
}
