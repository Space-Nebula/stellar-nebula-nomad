use soroban_sdk::{
    contract, contractimpl, contracttype, contracterror, symbol_short,
    Address, BytesN, Env, Vec,
};

use crate::nebula_explorer::NebulaLayout;

/// Compressed archive entry for historical nebula layouts
#[derive(Clone)]
#[contracttype]
pub struct NebulaArchive {
    pub archive_id: u64,
    pub layout: NebulaLayout,
    pub archived_at: u64,
    pub nebula_hash: BytesN<32>,
}

#[derive(Clone)]
#[contracttype]
pub enum DataKey {
    ArchiveCounter,
    Archive(u64),
    NebulaHashIndex(BytesN<32>),
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum ArchiveError {
    ArchiveNotFound = 1,
    BurstLimitExceeded = 2,
}

const MAX_ARCHIVE_BURST: u32 = 20;

#[contract]
pub struct NebulaArchiveContract;

#[contractimpl]
impl NebulaArchiveContract {
    /// Archive a nebula layout with timestamp and hash
    pub fn archive_nebula_layout(
        env: Env,
        layout: NebulaLayout,
    ) -> Result<u64, ArchiveError> {
        let archive_id = env
            .storage()
            .instance()
            .get(&DataKey::ArchiveCounter)
            .unwrap_or(0u64)
            + 1;

        let archived_at = env.ledger().timestamp();
        let nebula_hash = crate::nebula_explorer::compute_layout_hash(&env, &layout);

        let archive = NebulaArchive {
            archive_id,
            layout,
            archived_at,
            nebula_hash: nebula_hash.clone(),
        };

        env.storage()
            .persistent()
            .set(&DataKey::Archive(archive_id), &archive);
        env.storage()
            .persistent()
            .set(&DataKey::NebulaHashIndex(nebula_hash.clone()), &archive_id);
        env.storage()
            .instance()
            .set(&DataKey::ArchiveCounter, &archive_id);

        env.events().publish(
            (symbol_short!("archived"),),
            (archive_id, nebula_hash, archived_at),
        );

        Ok(archive_id)
    }

    /// Replay a historical nebula layout by archive ID
    pub fn replay_archive(env: Env, archive_id: u64) -> Result<NebulaArchive, ArchiveError> {
        env.storage()
            .persistent()
            .get(&DataKey::Archive(archive_id))
            .ok_or(ArchiveError::ArchiveNotFound)
    }

    /// Batch archive up to 20 layouts in one transaction
    pub fn batch_archive_layouts(
        env: Env,
        layouts: Vec<NebulaLayout>,
    ) -> Result<Vec<u64>, ArchiveError> {
        if layouts.len() > MAX_ARCHIVE_BURST {
            return Err(ArchiveError::BurstLimitExceeded);
        }

        let mut archive_ids = Vec::new(&env);
        for i in 0..layouts.len() {
            let layout = layouts.get(i).unwrap();
            let id = Self::archive_nebula_layout(env.clone(), layout)?;
            archive_ids.push_back(id);
        }

        Ok(archive_ids)
    }

    /// Query archive by nebula hash
    pub fn get_archive_by_hash(
        env: Env,
        nebula_hash: BytesN<32>,
    ) -> Result<NebulaArchive, ArchiveError> {
        let archive_id = env
            .storage()
            .persistent()
            .get(&DataKey::NebulaHashIndex(nebula_hash))
            .ok_or(ArchiveError::ArchiveNotFound)?;

        Self::replay_archive(env, archive_id)
    }

    /// Get total archive count
    pub fn get_archive_count(env: Env) -> u64 {
        env.storage()
            .instance()
            .get(&DataKey::ArchiveCounter)
            .unwrap_or(0u64)
    }
}
