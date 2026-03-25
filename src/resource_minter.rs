use crate::{CellType, NebulaLayout};
use crate::ship_nft::{DataKey as ShipDataKey, ShipNft};
use arrayvec::ArrayVec;
use soroban_sdk::{
    contracterror, contracttype, symbol_short, Address, Env, Symbol, Vec,
};

pub type AssetId = Symbol;

const ASSET_STELLAR_DUST: Symbol = symbol_short!("dust");
const ASSET_ASTEROID_ORE: Symbol = symbol_short!("ore");
const ASSET_GAS_UNITS: Symbol = symbol_short!("gas");
const ASSET_DARK_MATTER: Symbol = symbol_short!("dark");
const ASSET_EXOTIC_MATTER: Symbol = symbol_short!("exotic");
const ASSET_WORMHOLE_CORE: Symbol = symbol_short!("worm");

#[derive(Clone)]
#[contracttype]
pub enum ResourceKey {
    ResourceCounter,
    HarvestCounter,
    DexOfferCounter,
    ResourceBalance(Address, AssetId),
    DexOffer(u64),
}

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum HarvestError {
    ShipNotFound = 1,
    EmptyHarvest = 2,
    InvalidPrice = 3,
    AssetNotHarvested = 4,
    PriceOverflow = 5,
}

/// Resource data structure for in-game tradeable resources.
#[derive(Clone)]
#[contracttype]
pub struct Resource {
    pub id: u64,
    pub owner: Address,
    pub resource_type: u32,
    pub quantity: u32,
}

    /// Pro-rated APY yield calculation.
    ///
    /// ```text
    /// yield = principal × apy_bps / 10_000 × elapsed_ledgers / LEDGERS_PER_YEAR
    /// ```
    ///
    /// Integer division truncates fractional cosmic essence — this is intentional
    /// to keep the contract deterministic and avoid rounding exploits.
    fn calculate_yield(stake: &StakeRecord, current_ledger: u32, apy_bps: u32) -> i128 {
        let elapsed = current_ledger.saturating_sub(stake.last_claim_ledger) as i128;
        if elapsed == 0 {
            return 0;
        }
        stake.amount * (apy_bps as i128) * elapsed / (BPS_DENOM * LEDGERS_PER_YEAR)
    }
}
