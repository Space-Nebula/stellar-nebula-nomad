//! Predefined skin templates and marketplace

use soroban_sdk::{contracttype, symbol_short, Bytes, Env, Symbol, Vec};
use crate::ship_customization::{SkinRarity, ShipSkin};

#[contracttype]
#[derive(Clone)]
pub struct SkinTemplate {
    pub name: Symbol,
    pub rarity: SkinRarity,
    pub color_primary: u32,
    pub color_secondary: u32,
    pub price: i128,
}

/// Get all available skin templates
pub fn get_skin_templates(env: &Env) -> Vec<SkinTemplate> {
    let mut templates = Vec::new(env);
    
    // Common skins (10 templates)
    templates.push_back(SkinTemplate {
        name: symbol_short!("basic"),
        rarity: SkinRarity::Common,
        color_primary: 0xCCCCCC,
        color_secondary: 0x888888,
        price: 100,
    });
    templates.push_back(SkinTemplate {
        name: symbol_short!("red"),
        rarity: SkinRarity::Common,
        color_primary: 0xFF0000,
        color_secondary: 0x880000,
        price: 100,
    });
    templates.push_back(SkinTemplate {
        name: symbol_short!("blue"),
        rarity: SkinRarity::Common,
        color_primary: 0x0000FF,
        color_secondary: 0x000088,
        price: 100,
    });
    templates.push_back(SkinTemplate {
        name: symbol_short!("green"),
        rarity: SkinRarity::Common,
        color_primary: 0x00FF00,
        color_secondary: 0x008800,
        price: 100,
    });
    templates.push_back(SkinTemplate {
        name: symbol_short!("yellow"),
        rarity: SkinRarity::Common,
        color_primary: 0xFFFF00,
        color_secondary: 0x888800,
        price: 100,
    });
    templates.push_back(SkinTemplate {
        name: symbol_short!("purple"),
        rarity: SkinRarity::Common,
        color_primary: 0xFF00FF,
        color_secondary: 0x880088,
        price: 100,
    });
    templates.push_back(SkinTemplate {
        name: symbol_short!("cyan"),
        rarity: SkinRarity::Common,
        color_primary: 0x00FFFF,
        color_secondary: 0x008888,
        price: 100,
    });
    templates.push_back(SkinTemplate {
        name: symbol_short!("orange"),
        rarity: SkinRarity::Common,
        color_primary: 0xFF8800,
        color_secondary: 0x884400,
        price: 100,
    });
    templates.push_back(SkinTemplate {
        name: symbol_short!("pink"),
        rarity: SkinRarity::Common,
        color_primary: 0xFF88FF,
        color_secondary: 0x884488,
        price: 100,
    });
    templates.push_back(SkinTemplate {
        name: symbol_short!("white"),
        rarity: SkinRarity::Common,
        color_primary: 0xFFFFFF,
        color_secondary: 0xCCCCCC,
        price: 100,
    });
    
    // Rare skins (20 templates)
    templates.push_back(SkinTemplate {
        name: symbol_short!("flame"),
        rarity: SkinRarity::Rare,
        color_primary: 0xFF4400,
        color_secondary: 0xFF8800,
        price: 500,
    });
    templates.push_back(SkinTemplate {
        name: symbol_short!("ice"),
        rarity: SkinRarity::Rare,
        color_primary: 0x88FFFF,
        color_secondary: 0x44CCFF,
        price: 500,
    });
    templates.push_back(SkinTemplate {
        name: symbol_short!("toxic"),
        rarity: SkinRarity::Rare,
        color_primary: 0x88FF00,
        color_secondary: 0x44AA00,
        price: 500,
    });
    templates.push_back(SkinTemplate {
        name: symbol_short!("shadow"),
        rarity: SkinRarity::Rare,
        color_primary: 0x222222,
        color_secondary: 0x000000,
        price: 500,
    });
    templates.push_back(SkinTemplate {
        name: symbol_short!("gold"),
        rarity: SkinRarity::Rare,
        color_primary: 0xFFD700,
        color_secondary: 0xFFAA00,
        price: 500,
    });
    templates.push_back(SkinTemplate {
        name: symbol_short!("silver"),
        rarity: SkinRarity::Rare,
        color_primary: 0xC0C0C0,
        color_secondary: 0x808080,
        price: 500,
    });
    templates.push_back(SkinTemplate {
        name: symbol_short!("bronze"),
        rarity: SkinRarity::Rare,
        color_primary: 0xCD7F32,
        color_secondary: 0x8B4513,
        price: 500,
    });
    templates.push_back(SkinTemplate {
        name: symbol_short!("emerald"),
        rarity: SkinRarity::Rare,
        color_primary: 0x50C878,
        color_secondary: 0x228B22,
        price: 500,
    });
    templates.push_back(SkinTemplate {
        name: symbol_short!("ruby"),
        rarity: SkinRarity::Rare,
        color_primary: 0xE0115F,
        color_secondary: 0x9B111E,
        price: 500,
    });
    templates.push_back(SkinTemplate {
        name: symbol_short!("sapphire"),
        rarity: SkinRarity::Rare,
        color_primary: 0x0F52BA,
        color_secondary: 0x082567,
        price: 500,
    });
    templates.push_back(SkinTemplate {
        name: symbol_short!("amethyst"),
        rarity: SkinRarity::Rare,
        color_primary: 0x9966CC,
        color_secondary: 0x663399,
        price: 500,
    });
    templates.push_back(SkinTemplate {
        name: symbol_short!("topaz"),
        rarity: SkinRarity::Rare,
        color_primary: 0xFFC87C,
        color_secondary: 0xFF9933,
        price: 500,
    });
    templates.push_back(SkinTemplate {
        name: symbol_short!("pearl"),
        rarity: SkinRarity::Rare,
        color_primary: 0xF0EAD6,
        color_secondary: 0xE6D7B8,
        price: 500,
    });
    templates.push_back(SkinTemplate {
        name: symbol_short!("onyx"),
        rarity: SkinRarity::Rare,
        color_primary: 0x353839,
        color_secondary: 0x0F0F0F,
        price: 500,
    });
    templates.push_back(SkinTemplate {
        name: symbol_short!("jade"),
        rarity: SkinRarity::Rare,
        color_primary: 0x00A86B,
        color_secondary: 0x007850,
        price: 500,
    });
    templates.push_back(SkinTemplate {
        name: symbol_short!("coral"),
        rarity: SkinRarity::Rare,
        color_primary: 0xFF7F50,
        color_secondary: 0xFF6347,
        price: 500,
    });
    templates.push_back(SkinTemplate {
        name: symbol_short!("turquois"),
        rarity: SkinRarity::Rare,
        color_primary: 0x40E0D0,
        color_secondary: 0x00CED1,
        price: 500,
    });
    templates.push_back(SkinTemplate {
        name: symbol_short!("crimson"),
        rarity: SkinRarity::Rare,
        color_primary: 0xDC143C,
        color_secondary: 0x8B0000,
        price: 500,
    });
    templates.push_back(SkinTemplate {
        name: symbol_short!("indigo"),
        rarity: SkinRarity::Rare,
        color_primary: 0x4B0082,
        color_secondary: 0x2E0854,
        price: 500,
    });
    templates.push_back(SkinTemplate {
        name: symbol_short!("violet"),
        rarity: SkinRarity::Rare,
        color_primary: 0x8F00FF,
        color_secondary: 0x5F00A8,
        price: 500,
    });
    
    // Epic skins (15 templates)
    templates.push_back(SkinTemplate {
        name: symbol_short!("plasma"),
        rarity: SkinRarity::Epic,
        color_primary: 0xFF00FF,
        color_secondary: 0x00FFFF,
        price: 2000,
    });
    templates.push_back(SkinTemplate {
        name: symbol_short!("nebula"),
        rarity: SkinRarity::Epic,
        color_primary: 0x8844FF,
        color_secondary: 0xFF4488,
        price: 2000,
    });
    templates.push_back(SkinTemplate {
        name: symbol_short!("cosmic"),
        rarity: SkinRarity::Epic,
        color_primary: 0x4400FF,
        color_secondary: 0xFF0088,
        price: 2000,
    });
    templates.push_back(SkinTemplate {
        name: symbol_short!("aurora"),
        rarity: SkinRarity::Epic,
        color_primary: 0x00FF88,
        color_secondary: 0x88FF00,
        price: 2000,
    });
    templates.push_back(SkinTemplate {
        name: symbol_short!("galaxy"),
        rarity: SkinRarity::Epic,
        color_primary: 0x4400AA,
        color_secondary: 0xAA0044,
        price: 2000,
    });
    templates.push_back(SkinTemplate {
        name: symbol_short!("quantum"),
        rarity: SkinRarity::Epic,
        color_primary: 0x00AAFF,
        color_secondary: 0xFF00AA,
        price: 2000,
    });
    templates.push_back(SkinTemplate {
        name: symbol_short!("photon"),
        rarity: SkinRarity::Epic,
        color_primary: 0xFFFF00,
        color_secondary: 0x00FFFF,
        price: 2000,
    });
    templates.push_back(SkinTemplate {
        name: symbol_short!("neutron"),
        rarity: SkinRarity::Epic,
        color_primary: 0x0088FF,
        color_secondary: 0xFF8800,
        price: 2000,
    });
    templates.push_back(SkinTemplate {
        name: symbol_short!("pulsar"),
        rarity: SkinRarity::Epic,
        color_primary: 0xFF0044,
        color_secondary: 0x4400FF,
        price: 2000,
    });
    templates.push_back(SkinTemplate {
        name: symbol_short!("quasar"),
        rarity: SkinRarity::Epic,
        color_primary: 0x00FF44,
        color_secondary: 0xFF4400,
        price: 2000,
    });
    templates.push_back(SkinTemplate {
        name: symbol_short!("supernov"),
        rarity: SkinRarity::Epic,
        color_primary: 0xFFAA00,
        color_secondary: 0x00AAFF,
        price: 2000,
    });
    templates.push_back(SkinTemplate {
        name: symbol_short!("blackhol"),
        rarity: SkinRarity::Epic,
        color_primary: 0x000044,
        color_secondary: 0x440000,
        price: 2000,
    });
    templates.push_back(SkinTemplate {
        name: symbol_short!("wormhole"),
        rarity: SkinRarity::Epic,
        color_primary: 0x440088,
        color_secondary: 0x884400,
        price: 2000,
    });
    templates.push_back(SkinTemplate {
        name: symbol_short!("starborn"),
        rarity: SkinRarity::Epic,
        color_primary: 0xFFFFAA,
        color_secondary: 0xAAFFFF,
        price: 2000,
    });
    templates.push_back(SkinTemplate {
        name: symbol_short!("eclipse"),
        rarity: SkinRarity::Epic,
        color_primary: 0x220044,
        color_secondary: 0x442200,
        price: 2000,
    });
    
    // Legendary skins (5 templates)
    templates.push_back(SkinTemplate {
        name: symbol_short!("void"),
        rarity: SkinRarity::Legendary,
        color_primary: 0x000000,
        color_secondary: 0x8800FF,
        price: 10000,
    });
    templates.push_back(SkinTemplate {
        name: symbol_short!("stellar"),
        rarity: SkinRarity::Legendary,
        color_primary: 0xFFFFFF,
        color_secondary: 0xFFD700,
        price: 10000,
    });
    templates.push_back(SkinTemplate {
        name: symbol_short!("infinity"),
        rarity: SkinRarity::Legendary,
        color_primary: 0x0000FF,
        color_secondary: 0xFF0000,
        price: 10000,
    });
    templates.push_back(SkinTemplate {
        name: symbol_short!("eternity"),
        rarity: SkinRarity::Legendary,
        color_primary: 0xFFFFFF,
        color_secondary: 0x000000,
        price: 10000,
    });
    templates.push_back(SkinTemplate {
        name: symbol_short!("genesis"),
        rarity: SkinRarity::Legendary,
        color_primary: 0xFFD700,
        color_secondary: 0xFFFFFF,
        price: 10000,
    });
    
    templates
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_skin_templates_count() {
        let env = Env::default();
        let templates = get_skin_templates(&env);
        assert!(templates.len() >= 50);
    }

    #[test]
    fn test_rarity_pricing() {
        let env = Env::default();
        let templates = get_skin_templates(&env);
        
        for i in 0..templates.len() {
            let t = templates.get(i).unwrap();
            match t.rarity {
                SkinRarity::Common => assert_eq!(t.price, 100),
                SkinRarity::Rare => assert_eq!(t.price, 500),
                SkinRarity::Epic => assert_eq!(t.price, 2000),
                SkinRarity::Legendary => assert_eq!(t.price, 10000),
            }
        }
    }
}
