//! Ship visual customization and skin NFT system

use soroban_sdk::{contracterror, contracttype, symbol_short, Address, Bytes, Env, Symbol, Vec};

#[contracterror]
#[derive(Copy, Clone, Debug, PartialEq)]
#[repr(u32)]
pub enum SkinError {
    SkinNotFound = 1,
    NotOwner = 2,
    AlreadyApplied = 3,
    InvalidRarity = 4,
    SkinLimitReached = 5,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum SkinRarity {
    Common,
    Rare,
    Epic,
    Legendary,
}

#[contracttype]
#[derive(Clone)]
pub struct ShipSkin {
    pub skin_id: u64,
    pub owner: Address,
    pub name: Symbol,
    pub rarity: SkinRarity,
    pub color_primary: u32,
    pub color_secondary: u32,
    pub metadata: Bytes,
    pub tradeable: bool,
}

#[contracttype]
#[derive(Clone)]
pub enum SkinKey {
    SkinCounter,
    Skin(u64),
    ShipSkin(u64),
    OwnerSkins(Address),
}

fn next_skin_id(env: &Env) -> u64 {
    let current: u64 = env.storage().instance().get(&SkinKey::SkinCounter).unwrap_or(0);
    env.storage().instance().set(&SkinKey::SkinCounter, &(current + 1));
    current + 1
}

/// Mint a new skin NFT
pub fn mint_skin(
    env: &Env,
    owner: &Address,
    name: Symbol,
    rarity: SkinRarity,
    color_primary: u32,
    color_secondary: u32,
    metadata: Bytes,
) -> Result<ShipSkin, SkinError> {
    owner.require_auth();
    
    let skin_id = next_skin_id(env);
    let skin = ShipSkin {
        skin_id,
        owner: owner.clone(),
        name,
        rarity: rarity.clone(),
        color_primary,
        color_secondary,
        metadata,
        tradeable: true,
    };
    
    env.storage().persistent().set(&SkinKey::Skin(skin_id), &skin);
    
    let mut skins: Vec<u64> = env
        .storage()
        .persistent()
        .get(&SkinKey::OwnerSkins(owner.clone()))
        .unwrap_or_else(|| Vec::new(env));
    skins.push_back(skin_id);
    env.storage().persistent().set(&SkinKey::OwnerSkins(owner.clone()), &skins);
    
    env.events().publish(
        (symbol_short!("skin"), symbol_short!("minted")),
        (skin_id, owner.clone(), rarity),
    );
    
    Ok(skin)
}

/// Apply a skin to a ship
pub fn apply_skin(env: &Env, owner: &Address, ship_id: u64, skin_id: u64) -> Result<(), SkinError> {
    owner.require_auth();
    
    let skin: ShipSkin = env
        .storage()
        .persistent()
        .get(&SkinKey::Skin(skin_id))
        .ok_or(SkinError::SkinNotFound)?;
    
    if skin.owner != *owner {
        return Err(SkinError::NotOwner);
    }
    
    env.storage().persistent().set(&SkinKey::ShipSkin(ship_id), &skin_id);
    
    env.events().publish(
        (symbol_short!("skin"), symbol_short!("applied")),
        (ship_id, skin_id),
    );
    
    Ok(())
}

/// Get the skin applied to a ship
pub fn get_ship_skin(env: &Env, ship_id: u64) -> Option<u64> {
    env.storage().persistent().get(&SkinKey::ShipSkin(ship_id))
}

/// Get all skins owned by an address
pub fn get_owner_skins(env: &Env, owner: &Address) -> Vec<u64> {
    env.storage()
        .persistent()
        .get(&SkinKey::OwnerSkins(owner.clone()))
        .unwrap_or_else(|| Vec::new(env))
}

/// Transfer skin ownership
pub fn transfer_skin(env: &Env, skin_id: u64, new_owner: &Address) -> Result<ShipSkin, SkinError> {
    let mut skin: ShipSkin = env
        .storage()
        .persistent()
        .get(&SkinKey::Skin(skin_id))
        .ok_or(SkinError::SkinNotFound)?;
    
    skin.owner.require_auth();
    
    if !skin.tradeable {
        return Err(SkinError::AlreadyApplied);
    }
    
    let old_owner = skin.owner.clone();
    skin.owner = new_owner.clone();
    
    env.storage().persistent().set(&SkinKey::Skin(skin_id), &skin);
    
    env.events().publish(
        (symbol_short!("skin"), symbol_short!("xfer")),
        (skin_id, old_owner, new_owner.clone()),
    );
    
    Ok(skin)
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::testutils::Address as _;

    #[test]
    fn test_mint_and_apply_skin() {
        let env = Env::default();
        env.mock_all_auths();
        let owner = Address::generate(&env);
        let metadata = Bytes::from_array(&env, &[0u8; 4]);
        
        let skin = mint_skin(
            &env,
            &owner,
            symbol_short!("flame"),
            SkinRarity::Epic,
            0xFF0000,
            0x00FF00,
            metadata,
        ).unwrap();
        
        assert_eq!(skin.owner, owner);
        assert_eq!(skin.rarity, SkinRarity::Epic);
        
        apply_skin(&env, &owner, 1, skin.skin_id).unwrap();
        let applied = get_ship_skin(&env, 1);
        assert_eq!(applied, Some(skin.skin_id));
    }

    #[test]
    fn test_transfer_skin() {
        let env = Env::default();
        env.mock_all_auths();
        let owner = Address::generate(&env);
        let new_owner = Address::generate(&env);
        let metadata = Bytes::from_array(&env, &[0u8; 4]);
        
        let skin = mint_skin(
            &env,
            &owner,
            symbol_short!("cosmic"),
            SkinRarity::Legendary,
            0x0000FF,
            0xFFFF00,
            metadata,
        ).unwrap();
        
        let transferred = transfer_skin(&env, skin.skin_id, &new_owner).unwrap();
        assert_eq!(transferred.owner, new_owner);
    }
}
