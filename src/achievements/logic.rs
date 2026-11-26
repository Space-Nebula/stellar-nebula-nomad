use super::types::*;
use super::catalog_defs::*;
use crate::achievement_engine::AchievementError;
use crate::player_profile::get_profile_by_owner;
use crate::reputation::get_reputation_score;
use crate::ship_registry::get_ships_by_owner;
use soroban_sdk::{symbol_short, Address, Env, Symbol, Vec};


pub fn query_progress(
    env: &Env,
    player: &Address,
    achievement_id: u64,
) -> Result<AchievementProgress, AchievementsError> {
    let profile =
        get_profile_by_owner(env, player).map_err(|_| AchievementsError::ProfileNotFound)?;

    let ships = get_ships_by_owner(env, player);
    let ship_count = ships.len() as u32;

    let def = get_achievement_def(env, achievement_id)?;

    let unlocked = env
        .storage()
        .persistent()
        .has(&AchievementKey::PlayerAchievement(
            player.clone(),
            achievement_id,
        ));

    let unlocked_at = if unlocked {
        env.storage()
            .persistent()
            .get(&AchievementKey::PlayerAchievement(
                player.clone(),
                achievement_id,
            ))
            .unwrap_or(0u64)
    } else {
        0
    };

    let pct = compute_progress_pct(
        &def,
        profile.total_scans,
        profile.essence_earned,
        ship_count,
    );

    Ok(AchievementProgress {
        achievement_id,
        title: def.title,
        description: if def.is_secret && !unlocked {
            String::from_str(env, "???")
        } else {
            def.description
        },
        category: def.category,
        tier: def.tier,
        unlocked,
        eligible: pct >= 100 && !unlocked,
        progress_pct: pct,
        points: def.points,
        is_secret: def.is_secret,
        unlocked_at,
    })
}

pub fn query_all_progress(
    env: &Env,
    player: &Address,
) -> Result<Vec<AchievementProgress>, AchievementsError> {
    let mut progress = Vec::new(env);

    for id in 1..=TOTAL_ACHIEVEMENTS {
        if let Ok(p) = query_progress(env, player, id) {
            progress.push_back(p);
        }
    }

    Ok(progress)
}

pub fn query_category_progress(
    env: &Env,
    player: &Address,
    category: AchievementCategory,
) -> Result<Vec<AchievementProgress>, AchievementsError> {
    let mut progress = Vec::new(env);

    for id in 1..=TOTAL_ACHIEVEMENTS {
        if let Ok(def) = get_achievement_def(env, id) {
            if def.category as u32 == category as u32 {
                if let Ok(p) = query_progress(env, player, id) {
                    progress.push_back(p);
                }
            }
        }
    }

    Ok(progress)
}

// ─── Unlock Functions ────────────────────────────────────────────────────────

pub fn try_unlock(
    env: &Env,
    player: &Address,
    achievement_id: u64,
) -> Result<u64, AchievementError> {
    let badge = unlock_achievement(env, player.clone(), achievement_id)?;

    // Update player stats
    increment_player_stats(env, player, achievement_id)?;

    // Update leaderboard
    update_leaderboard(env, player);

    // Create notification
    create_notification(env, player, achievement_id);

    // Integrate with reputation
    integrate_reputation(env, player, achievement_id);

    // Emit event
    emit_achievement_event(env, player, achievement_id, badge.badge_id);

    // Check for meta-achievements
    check_meta_achievements(env, player)?;

    Ok(badge.badge_id)
}

// ─── Player Stats Management ─────────────────────────────────────────────────

fn increment_player_stats(
    env: &Env,
    player: &Address,
    achievement_id: u64,
) -> Result<(), AchievementsError> {
    let key = AchievementStorageKey::AchievementPlayerStats(player.clone());
    let mut stats: PlayerAchievementStats =
        env.storage()
            .persistent()
            .get(&key)
            .unwrap_or_else(|| PlayerAchievementStats {
                player: player.clone(),
                total_unlocked: 0,
                total_points: 0,
                bronze_count: 0,
                silver_count: 0,
                gold_count: 0,
                platinum_count: 0,
                secret_count: 0,
                meta_count: 0,
                completion_pct: 0,
                leaderboard_rank: 0,
            });

    let def = get_achievement_def(env, achievement_id)?;

    stats.total_unlocked += 1;
    stats.total_points += def.points;

    match def.tier {
        AchievementTier::Bronze => stats.bronze_count += 1,
        AchievementTier::Silver => stats.silver_count += 1,
        AchievementTier::Gold => stats.gold_count += 1,
        AchievementTier::Platinum => stats.platinum_count += 1,
    }

    if def.is_secret {
        stats.secret_count += 1;
    }

    if def.is_meta {
        stats.meta_count += 1;
    }

    stats.completion_pct = (stats.total_unlocked * 100) / (TOTAL_ACHIEVEMENTS as u32);

    env.storage().persistent().set(&key, &stats);

    Ok(())
}

pub fn get_player_stats(
    env: &Env,
    player: &Address,
) -> Result<PlayerAchievementStats, AchievementsError> {
    let key = AchievementStorageKey::AchievementPlayerStats(player.clone());
    env.storage()
        .persistent()
        .get(&key)
        .ok_or(AchievementsError::ProfileNotFound)
}

// ─── Leaderboard Management ──────────────────────────────────────────────────

fn update_leaderboard(env: &Env, player: &Address) {
    let stats = get_player_stats(env, player).unwrap_or_else(|_| PlayerAchievementStats {
        player: player.clone(),
        total_unlocked: 0,
        total_points: 0,
        bronze_count: 0,
        silver_count: 0,
        gold_count: 0,
        platinum_count: 0,
        secret_count: 0,
        meta_count: 0,
        completion_pct: 0,
        leaderboard_rank: 0,
    });

    // Update count-based leaderboard
    env.storage().persistent().set(
        &LeaderboardKey::PlayerScore(player.clone()),
        &stats.total_unlocked,
    );

    // Update points-based leaderboard
    env.storage().persistent().set(
        &LeaderboardKey::PlayerPoints(player.clone()),
        &stats.total_points,
    );

    record_leaderboard_entry(env, player, stats.total_unlocked, stats.total_points);
}

fn record_leaderboard_entry(env: &Env, player: &Address, count: u32, points: u32) {
    let mut entries: Vec<AchievementLeaderboardEntry> = env
        .storage()
        .persistent()
        .get(&LeaderboardKey::TopEntriesByPoints)
        .unwrap_or_else(|| Vec::new(env));

    // Remove existing entry
    let mut without_player: Vec<AchievementLeaderboardEntry> = Vec::new(env);
    for i in 0..entries.len() {
        if let Some(e) = entries.get(i) {
            if e.player != *player {
                without_player.push_back(e);
            }
        }
    }

    // Insert new entry
    let new_entry = AchievementLeaderboardEntry {
        player: player.clone(),
        achievement_count: count,
        achievement_points: points,
        rank: 0,
    };

    let mut sorted: Vec<AchievementLeaderboardEntry> = Vec::new(env);
    let mut inserted = false;

    for i in 0..without_player.len() {
        if let Some(e) = without_player.get(i) {
            if !inserted && points >= e.achievement_points {
                sorted.push_back(new_entry.clone());
                inserted = true;
            }
            sorted.push_back(e);
        }
    }

    if !inserted {
        sorted.push_back(new_entry);
    }

    // Cap at 100 entries and assign ranks
    let mut ranked: Vec<AchievementLeaderboardEntry> = Vec::new(env);
    for i in 0..sorted.len().min(100) {
        if let Some(mut e) = sorted.get(i) {
            e.rank = i + 1;
            ranked.push_back(e);
        }
    }

    env.storage()
        .persistent()
        .set(&LeaderboardKey::TopEntriesByPoints, &ranked);
}

pub fn get_leaderboard(env: &Env) -> Vec<AchievementLeaderboardEntry> {
    env.storage()
        .persistent()
        .get(&LeaderboardKey::TopEntriesByPoints)
        .unwrap_or_else(|| Vec::new(env))
}

// ─── Notifications ───────────────────────────────────────────────────────────

fn create_notification(env: &Env, player: &Address, achievement_id: u64) {
    let def = match get_achievement_def(env, achievement_id) {
        Ok(d) => d,
        Err(_) => return,
    };

    let count_key = AchievementStorageKey::NotificationCount(player.clone());
    let count: u64 = env.storage().persistent().get(&count_key).unwrap_or(0);
    let new_count = count + 1;

    let notification = AchievementNotification {
        player: player.clone(),
        achievement_id,
        title: def.title,
        points_earned: def.points,
        timestamp: env.ledger().timestamp(),
        is_secret: def.is_secret,
    };

    env.storage().persistent().set(
        &AchievementStorageKey::Notification(player.clone(), new_count),
        &notification,
    );

    env.storage().persistent().set(&count_key, &new_count);

    env.events().publish(
        (symbol_short!("ach_notif"), player.clone()),
        (achievement_id, def.points, def.is_secret),
    );
}

pub fn get_notifications(env: &Env, player: &Address) -> Vec<AchievementNotification> {
    let count_key = AchievementStorageKey::NotificationCount(player.clone());
    let count: u64 = env.storage().persistent().get(&count_key).unwrap_or(0);

    let mut notifications = Vec::new(env);

    for i in 1..=count.min(50) {
        if let Some(notif) = env
            .storage()
            .persistent()
            .get::<AchievementStorageKey, AchievementNotification>(
                &AchievementStorageKey::Notification(player.clone(), count - i + 1),
            )
        {
            notifications.push_back(notif);
        }
    }

    notifications
}

// ─── Reputation Integration ──────────────────────────────────────────────────

fn integrate_reputation(env: &Env, player: &Address, achievement_id: u64) {
    let def = match get_achievement_def(env, achievement_id) {
        Ok(d) => d,
        Err(_) => return,
    };

    let reputation_points = match def.tier {
        AchievementTier::Bronze => 1,
        AchievementTier::Silver => 2,
        AchievementTier::Gold => 4,
        AchievementTier::Platinum => 8,
    };

    // Store reputation gain for external reputation system
    env.events().publish(
        (symbol_short!("rep_gain"), player.clone()),
        (ReputationSource::GameAchievement as u32, reputation_points),
    );
}

// ─── Meta-Achievements ───────────────────────────────────────────────────────

fn check_meta_achievements(env: &Env, player: &Address) -> Result<(), AchievementError> {
    let stats = get_player_stats(env, player).map_err(|_| AchievementError::ProfileNotFound)?;

    // Check bronze collector
    if stats.bronze_count >= 50 && !is_unlocked(env, player, 205) {
        let _ = try_unlock(env, player, 205);
    }

    // Check silver collector
    if stats.silver_count >= 40 && !is_unlocked(env, player, 206) {
        let _ = try_unlock(env, player, 206);
    }

    // Check gold collector
    if stats.gold_count >= 30 && !is_unlocked(env, player, 207) {
        let _ = try_unlock(env, player, 207);
    }

    // Check platinum collector
    if stats.platinum_count >= 20 && !is_unlocked(env, player, 208) {
        let _ = try_unlock(env, player, 208);
    }

    // Check completionist milestones
    if stats.total_unlocked >= 50 && !is_unlocked(env, player, 211) {
        let _ = try_unlock(env, player, 211);
    }

    if stats.total_unlocked >= 100 && !is_unlocked(env, player, 212) {
        let _ = try_unlock(env, player, 212);
    }

    if stats.total_unlocked >= 200 && !is_unlocked(env, player, 214) {
        let _ = try_unlock(env, player, 214);
    }

    // Check ultimate completionist
    if stats.total_unlocked >= 250 && !is_unlocked(env, player, 215) {
        let _ = try_unlock(env, player, 215);
    }

    // Check points milestones
    if stats.total_points >= 1000 && !is_unlocked(env, player, 221) {
        let _ = try_unlock(env, player, 221);
    }

    if stats.total_points >= 5000 && !is_unlocked(env, player, 222) {
        let _ = try_unlock(env, player, 222);
    }

    if stats.total_points >= 10000 && !is_unlocked(env, player, 223) {
        let _ = try_unlock(env, player, 223);
    }

    Ok(())
}

fn is_unlocked(env: &Env, player: &Address, achievement_id: u64) -> bool {
    env.storage()
        .persistent()
        .has(&AchievementKey::PlayerAchievement(
            player.clone(),
            achievement_id,
        ))
}

// ─── Events ──────────────────────────────────────────────────────────────────

fn emit_achievement_event(env: &Env, player: &Address, achievement_id: u64, badge_id: u64) {
    let def = match get_achievement_def(env, achievement_id) {
        Ok(d) => d,
        Err(_) => return,
    };

    env.events().publish(
        (symbol_short!("ach_unlck"), player.clone()),
        (achievement_id, badge_id, def.tier as u32, def.points),
    );
}

// ─── Helper Functions ────────────────────────────────────────────────────────

fn compute_progress_pct(def: &AchievementDef, scans: u32, essence: i128, ships: u32) -> u32 {
    let mut pct = 100u32;

    if def.min_scans > 0 {
        pct = pct.min((scans.saturating_mul(100) / def.min_scans).min(100));
    }

    if def.min_essence > 0 {
        let essence_u = if essence < 0 { 0 } else { essence as u128 };
        let required = def.min_essence as u128;
        pct = pct.min(((essence_u.saturating_mul(100)) / required).min(100) as u32);
    }

    if def.min_ships > 0 {
        pct = pct.min((ships.saturating_mul(100) / def.min_ships).min(100));
    }

    pct
}

// ─── Batch Operations ────────────────────────────────────────────────────────

pub fn batch_check_and_unlock(env: &Env, player: &Address) -> Result<Vec<u64>, AchievementError> {
    let profile =
        get_profile_by_owner(env, player).map_err(|_| AchievementError::ProfileNotFound)?;

    let ships = get_ships_by_owner(env, player);
    let ship_count = ships.len() as u32;

    let mut unlocked_ids = Vec::new(env);

    for id in 1..=TOTAL_ACHIEVEMENTS {
        if is_unlocked(env, player, id) {
            continue;
        }

        if let Ok(def) = get_achievement_def(env, id) {
            let pct = compute_progress_pct(
                &def,
                profile.total_scans,
                profile.essence_earned,
                ship_count,
            );

            if pct >= 100 {
                if let Ok(badge_id) = try_unlock(env, player, id) {
                    unlocked_ids.push_back(id);
                }
            }
        }
    }

    Ok(unlocked_ids)
}
