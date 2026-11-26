//! # Theme Customization (#700)
//!
//! On-chain half of the theme system. The frontend (`frontend/src/theme`) owns
//! rendering; this module owns what has to survive across devices:
//!
//! * **Ship themes** — ten preset nebula themes applied per ship
//!   ([`apply_theme`] / [`get_theme`]).
//! * **Player theme preferences** — preset, dark/light/system mode,
//!   accessibility profile, custom colour overrides, reduced motion and font
//!   scale ([`set_theme_preferences`] / [`get_theme_preferences`]). Saved per
//!   player so a nomad's look follows them to any client.
//!
//! Colours are stored as packed `0xRRGGBB` integers. Contrast checking happens
//! in the client (WCAG maths needs floating point); the contract only enforces
//! well-formed values and bounded storage.

use soroban_sdk::{contracterror, contracttype, symbol_short, Address, Env, Symbol, Vec};

/// Number of preset themes (`nebula1` … `nebula10`).
pub const PRESET_THEME_COUNT: u32 = 10;
/// Maximum number of custom colour overrides a player may store.
pub const MAX_CUSTOM_COLORS: u32 = 6;
/// Smallest accepted font scale, in percent of the default size.
pub const MIN_FONT_SCALE_PCT: u32 = 75;
/// Largest accepted font scale, in percent of the default size.
pub const MAX_FONT_SCALE_PCT: u32 = 200;
/// Largest packed `0xRRGGBB` value.
pub const MAX_RGB: u32 = 0x00FF_FFFF;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum ThemeError {
    InvalidTheme = 1,
    Unauthorized = 2,
    ShipNotFound = 3,
    /// A custom colour is not a valid `0xRRGGBB` value.
    InvalidColor = 4,
    /// More than [`MAX_CUSTOM_COLORS`] overrides, or a slot listed twice.
    TooManyCustomColors = 5,
    /// Font scale outside `MIN_FONT_SCALE_PCT..=MAX_FONT_SCALE_PCT`.
    InvalidFontScale = 6,
    /// Duplicate colour slot in one preference update.
    DuplicateColorSlot = 7,
}

impl crate::error_standard::StandardContractError for ThemeError {
    fn descriptor(self) -> crate::error_standard::ErrorDescriptor {
        use crate::error_standard::ErrorKind;
        let (kind, retryable) = match self {
            Self::InvalidTheme => (ErrorKind::Validation, false),
            Self::Unauthorized => (ErrorKind::Authorization, false),
            Self::ShipNotFound => (ErrorKind::NotFound, false),
            Self::InvalidColor
            | Self::TooManyCustomColors
            | Self::InvalidFontScale
            | Self::DuplicateColorSlot => (ErrorKind::Validation, false),
        };
        crate::error_standard::ErrorDescriptor {
            module: "theme_customizer",
            code: self as u32,
            kind,
            retryable,
        }
    }
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ThemePreview {
    pub name: Symbol,
    pub colors: Vec<Symbol>, // Hex color codes
    pub particles: Symbol,
}

pub fn generate_theme_preview(env: Env, theme_id: Symbol) -> Result<ThemePreview, ThemeError> {
    match theme_id {
        s if s == symbol_short!("nebula1") => Ok(ThemePreview {
            name: symbol_short!("Cosmic"),
            colors: Vec::from_array(&env, [symbol_short!("FF00FF"), symbol_short!("00FFFF")]),
            particles: symbol_short!("Stardust"),
        }),
        s if s == symbol_short!("nebula2") => Ok(ThemePreview {
            name: symbol_short!("Void"),
            colors: Vec::from_array(&env, [symbol_short!("000000"), symbol_short!("444444")]),
            particles: symbol_short!("dark_mtr"),
        }),
        s if s == symbol_short!("nebula3") => Ok(ThemePreview {
            name: symbol_short!("Nova"),
            colors: Vec::from_array(&env, [symbol_short!("FFA500"), symbol_short!("FF4500")]),
            particles: symbol_short!("Flare"),
        }),
        s if s == symbol_short!("nebula4") => Ok(ThemePreview {
            name: symbol_short!("Quasar"),
            colors: Vec::from_array(&env, [symbol_short!("0000FF"), symbol_short!("FFFFFF")]),
            particles: symbol_short!("Beams"),
        }),
        s if s == symbol_short!("nebula5") => Ok(ThemePreview {
            name: symbol_short!("Supernova"),
            colors: Vec::from_array(&env, [symbol_short!("FF0000"), symbol_short!("FFFF00")]),
            particles: symbol_short!("Shockwave"),
        }),
        s if s == symbol_short!("nebula6") => Ok(ThemePreview {
            name: symbol_short!("Wormhole"),
            colors: Vec::from_array(&env, [symbol_short!("A020F0"), symbol_short!("000000")]),
            particles: symbol_short!("Vortex"),
        }),
        s if s == symbol_short!("nebula7") => Ok(ThemePreview {
            name: symbol_short!("BlackHole"),
            colors: Vec::from_array(&env, [symbol_short!("000000"), symbol_short!("111111")]),
            particles: symbol_short!("snglrty"),
        }),
        s if s == symbol_short!("nebula8") => Ok(ThemePreview {
            name: symbol_short!("Aurora"),
            colors: Vec::from_array(&env, [symbol_short!("00FF00"), symbol_short!("B026FF")]),
            particles: symbol_short!("Borealis"),
        }),
        s if s == symbol_short!("nebula9") => Ok(ThemePreview {
            name: symbol_short!("Eclipse"),
            colors: Vec::from_array(&env, [symbol_short!("CCCCCC"), symbol_short!("000000")]),
            particles: symbol_short!("Corral"),
        }),
        s if s == symbol_short!("nebula10") => Ok(ThemePreview {
            name: symbol_short!("Meteor"),
            colors: Vec::from_array(&env, [symbol_short!("FFD700"), symbol_short!("8B4513")]),
            particles: symbol_short!("Trails"),
        }),
        _ => Err(ThemeError::InvalidTheme),
    }
}

pub fn apply_theme(
    env: Env,
    owner: Address,
    ship_id: u64,
    theme_id: Symbol,
) -> Result<(), ThemeError> {
    owner.require_auth();

    // In a real scenario, we'd check if the owner owns the ship using ship_nft module.
    // For this prototype, we'll assume the caller must be authorized and ship exists.

    // Validate theme first
    let _ = generate_theme_preview(env.clone(), theme_id.clone())?;

    // Store ship-to-theme association
    env.storage()
        .persistent()
        .set(&(symbol_short!("theme"), ship_id), &theme_id);

    env.events().publish(
        (symbol_short!("theme"), symbol_short!("applied")),
        (ship_id, theme_id),
    );

    Ok(())
}

pub fn get_theme(env: Env, ship_id: u64) -> Option<Symbol> {
    env.storage()
        .persistent()
        .get(&(symbol_short!("theme"), ship_id))
}

/// Preset theme IDs in display order.
pub fn list_theme_ids(env: &Env) -> Vec<Symbol> {
    Vec::from_array(
        env,
        [
            symbol_short!("nebula1"),
            symbol_short!("nebula2"),
            symbol_short!("nebula3"),
            symbol_short!("nebula4"),
            symbol_short!("nebula5"),
            symbol_short!("nebula6"),
            symbol_short!("nebula7"),
            symbol_short!("nebula8"),
            symbol_short!("nebula9"),
            symbol_short!("nebula10"),
        ],
    )
}

// ─── Player theme preferences ────────────────────────────────────────────────

/// Colour scheme. `System` follows the device's `prefers-color-scheme`.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum ThemeMode {
    Dark = 0,
    Light = 1,
    System = 2,
}

/// Accessibility palette the client should render with. Anything other than
/// `Standard` takes priority over the preset's own palette.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum AccessibilityProfile {
    Standard = 0,
    /// Maximum-contrast palette (7:1 text, WCAG AAA).
    HighContrast = 1,
    /// Red/green-safe palette (deuteranopia and protanopia).
    RedGreenSafe = 2,
    /// Blue/yellow-safe palette (tritanopia).
    BlueYellowSafe = 3,
}

/// Themeable colour roles a player may override.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum ColorSlot {
    Background = 0,
    Surface = 1,
    Text = 2,
    Primary = 3,
    Accent = 4,
    Border = 5,
}

/// One custom colour: a role and a packed `0xRRGGBB` value.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ColorOverride {
    pub slot: ColorSlot,
    pub rgb: u32,
}

/// A player's saved theme settings.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ThemePreferences {
    /// One of [`list_theme_ids`].
    pub theme_id: Symbol,
    pub mode: ThemeMode,
    pub accessibility: AccessibilityProfile,
    /// At most [`MAX_CUSTOM_COLORS`], one per slot.
    pub custom_colors: Vec<ColorOverride>,
    pub reduced_motion: bool,
    /// Percent of the default font size.
    pub font_scale_pct: u32,
    /// Ledger timestamp of the last save (set by the contract).
    pub updated_at: u64,
}

#[contracttype]
#[derive(Clone)]
enum ThemeKey {
    Prefs(Address),
}

/// Settings a player gets before saving anything.
pub fn default_theme_preferences(env: &Env) -> ThemePreferences {
    ThemePreferences {
        theme_id: symbol_short!("nebula1"),
        mode: ThemeMode::System,
        accessibility: AccessibilityProfile::Standard,
        custom_colors: Vec::new(env),
        reduced_motion: false,
        font_scale_pct: 100,
        updated_at: 0,
    }
}

/// Check a preference set without storing it.
pub fn validate_theme_preferences(env: &Env, prefs: &ThemePreferences) -> Result<(), ThemeError> {
    generate_theme_preview(env.clone(), prefs.theme_id.clone())?;

    if prefs.font_scale_pct < MIN_FONT_SCALE_PCT || prefs.font_scale_pct > MAX_FONT_SCALE_PCT {
        return Err(ThemeError::InvalidFontScale);
    }
    if prefs.custom_colors.len() > MAX_CUSTOM_COLORS {
        return Err(ThemeError::TooManyCustomColors);
    }

    let mut seen: u32 = 0;
    for color in prefs.custom_colors.iter() {
        if color.rgb > MAX_RGB {
            return Err(ThemeError::InvalidColor);
        }
        let bit = 1u32 << (color.slot as u32);
        if seen & bit != 0 {
            return Err(ThemeError::DuplicateColorSlot);
        }
        seen |= bit;
    }
    Ok(())
}

/// Save `player`'s theme preferences. Returns the stored value (with
/// `updated_at` stamped). Emits `theme/prefs`.
pub fn set_theme_preferences(
    env: &Env,
    player: Address,
    prefs: ThemePreferences,
) -> Result<ThemePreferences, ThemeError> {
    player.require_auth();
    validate_theme_preferences(env, &prefs)?;

    let mut stored = prefs;
    stored.updated_at = env.ledger().timestamp();
    env.storage()
        .persistent()
        .set(&ThemeKey::Prefs(player.clone()), &stored);

    env.events().publish(
        (symbol_short!("theme"), symbol_short!("prefs")),
        (
            player,
            stored.theme_id.clone(),
            stored.mode,
            stored.accessibility,
        ),
    );
    Ok(stored)
}

/// `player`'s saved preferences, or the defaults if they never saved any.
pub fn get_theme_preferences(env: &Env, player: Address) -> ThemePreferences {
    env.storage()
        .persistent()
        .get(&ThemeKey::Prefs(player))
        .unwrap_or_else(|| default_theme_preferences(env))
}

/// True when `player` has saved preferences.
pub fn has_theme_preferences(env: &Env, player: Address) -> bool {
    env.storage().persistent().has(&ThemeKey::Prefs(player))
}

/// Delete `player`'s saved preferences, returning them to the defaults.
pub fn reset_theme_preferences(env: &Env, player: Address) {
    player.require_auth();
    env.storage()
        .persistent()
        .remove(&ThemeKey::Prefs(player.clone()));
    env.events()
        .publish((symbol_short!("theme"), symbol_short!("reset")), player);
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{contract, contractimpl, testutils::Address as _, testutils::Ledger};

    #[contract]
    struct Stub;
    #[contractimpl]
    impl Stub {}

    fn make_env() -> (Env, Address) {
        let env = Env::default();
        env.mock_all_auths();
        let id = env.register(Stub, ());
        (env, id)
    }

    #[test]
    fn test_generate_theme_preview_rejects_unknown_theme() {
        let env = Env::default();
        let result = generate_theme_preview(env.clone(), symbol_short!("bogus"));
        assert_eq!(result, Err(ThemeError::InvalidTheme));
    }

    #[test]
    fn test_generate_theme_preview_first_and_last_boundary() {
        let env = Env::default();
        let first = generate_theme_preview(env.clone(), symbol_short!("nebula1")).unwrap();
        assert_eq!(first.name, symbol_short!("Cosmic"));

        let last = generate_theme_preview(env.clone(), symbol_short!("nebula10")).unwrap();
        assert_eq!(last.name, symbol_short!("Meteor"));
    }

    #[test]
    fn test_get_theme_missing_ship_returns_none() {
        let (env, contract_id) = make_env();
        env.as_contract(&contract_id, || {
            assert!(get_theme(env.clone(), 12345).is_none());
        });
    }

    #[test]
    fn test_apply_theme_rejects_invalid_theme_and_persists_nothing() {
        let (env, contract_id) = make_env();
        let owner = Address::generate(&env);
        env.as_contract(&contract_id, || {
            let result = apply_theme(env.clone(), owner, 1, symbol_short!("bogus"));
            assert_eq!(result, Err(ThemeError::InvalidTheme));
            assert!(get_theme(env.clone(), 1).is_none());
        });
    }

    #[test]
    fn test_apply_theme_then_get_theme_roundtrip() {
        let (env, contract_id) = make_env();
        let owner = Address::generate(&env);
        env.as_contract(&contract_id, || {
            apply_theme(env.clone(), owner, 7, symbol_short!("nebula3")).unwrap();
            assert_eq!(get_theme(env.clone(), 7), Some(symbol_short!("nebula3")));
        });
    }

    #[test]
    fn test_all_ten_presets_resolve() {
        let env = Env::default();
        let ids = list_theme_ids(&env);
        assert_eq!(ids.len(), PRESET_THEME_COUNT);
        for id in ids.iter() {
            let preview = generate_theme_preview(env.clone(), id).unwrap();
            assert_eq!(preview.colors.len(), 2);
        }
    }

    fn prefs(env: &Env) -> ThemePreferences {
        ThemePreferences {
            theme_id: symbol_short!("nebula8"),
            mode: ThemeMode::Light,
            accessibility: AccessibilityProfile::HighContrast,
            custom_colors: Vec::from_array(
                env,
                [
                    ColorOverride {
                        slot: ColorSlot::Primary,
                        rgb: 0x2563EB,
                    },
                    ColorOverride {
                        slot: ColorSlot::Accent,
                        rgb: 0xFFFFFF,
                    },
                ],
            ),
            reduced_motion: true,
            font_scale_pct: 125,
            updated_at: 0,
        }
    }

    #[test]
    fn test_preferences_default_when_unset() {
        let (env, contract_id) = make_env();
        let player = Address::generate(&env);
        env.as_contract(&contract_id, || {
            assert!(!has_theme_preferences(&env, player.clone()));
            let p = get_theme_preferences(&env, player);
            assert_eq!(p, default_theme_preferences(&env));
            assert_eq!(p.mode, ThemeMode::System);
        });
    }

    #[test]
    fn test_preferences_persist_and_stamp_time() {
        let (env, contract_id) = make_env();
        env.ledger().with_mut(|l| l.timestamp = 5_000);
        let player = Address::generate(&env);
        env.as_contract(&contract_id, || {
            let saved = set_theme_preferences(&env, player.clone(), prefs(&env)).unwrap();
            assert_eq!(saved.updated_at, 5_000);
            let loaded = get_theme_preferences(&env, player.clone());
            assert_eq!(loaded, saved);
            assert_eq!(loaded.accessibility, AccessibilityProfile::HighContrast);
            assert_eq!(loaded.custom_colors.len(), 2);
        });
    }

    #[test]
    fn test_preferences_are_per_player() {
        let (env, contract_id) = make_env();
        let a = Address::generate(&env);
        let b = Address::generate(&env);
        env.as_contract(&contract_id, || {
            set_theme_preferences(&env, a.clone(), prefs(&env)).unwrap();
            assert_eq!(
                get_theme_preferences(&env, b),
                default_theme_preferences(&env)
            );
        });
    }

    #[test]
    fn test_reset_returns_to_defaults() {
        let (env, contract_id) = make_env();
        let player = Address::generate(&env);
        env.as_contract(&contract_id, || {
            set_theme_preferences(&env, player.clone(), prefs(&env)).unwrap();
        });
        env.as_contract(&contract_id, || {
            reset_theme_preferences(&env, player.clone());
            assert!(!has_theme_preferences(&env, player.clone()));
            assert_eq!(
                get_theme_preferences(&env, player),
                default_theme_preferences(&env)
            );
        });
    }

    #[test]
    fn test_preferences_validation() {
        let (env, contract_id) = make_env();
        let player = Address::generate(&env);
        env.as_contract(&contract_id, || {
            let mut bad_theme = prefs(&env);
            bad_theme.theme_id = symbol_short!("bogus");
            assert_eq!(
                set_theme_preferences(&env, player.clone(), bad_theme),
                Err(ThemeError::InvalidTheme)
            );

            let mut bad_color = prefs(&env);
            bad_color.custom_colors = Vec::from_array(
                &env,
                [ColorOverride {
                    slot: ColorSlot::Text,
                    rgb: 0x0100_0000,
                }],
            );
            assert_eq!(
                validate_theme_preferences(&env, &bad_color),
                Err(ThemeError::InvalidColor)
            );

            let mut dup = prefs(&env);
            dup.custom_colors = Vec::from_array(
                &env,
                [
                    ColorOverride {
                        slot: ColorSlot::Text,
                        rgb: 0,
                    },
                    ColorOverride {
                        slot: ColorSlot::Text,
                        rgb: 1,
                    },
                ],
            );
            assert_eq!(
                validate_theme_preferences(&env, &dup),
                Err(ThemeError::DuplicateColorSlot)
            );

            let mut too_many = prefs(&env);
            let mut colors = Vec::new(&env);
            for _ in 0..(MAX_CUSTOM_COLORS + 1) {
                colors.push_back(ColorOverride {
                    slot: ColorSlot::Text,
                    rgb: 0,
                });
            }
            too_many.custom_colors = colors;
            assert_eq!(
                validate_theme_preferences(&env, &too_many),
                Err(ThemeError::TooManyCustomColors)
            );

            for scale in [MIN_FONT_SCALE_PCT - 1, MAX_FONT_SCALE_PCT + 1] {
                let mut p = prefs(&env);
                p.font_scale_pct = scale;
                assert_eq!(
                    validate_theme_preferences(&env, &p),
                    Err(ThemeError::InvalidFontScale)
                );
            }
            // Nothing was stored by the failed attempts.
            assert!(!has_theme_preferences(&env, player));
        });
    }

    #[test]
    fn test_every_slot_can_be_overridden_once() {
        let env = Env::default();
        let mut p = prefs(&env);
        p.custom_colors = Vec::from_array(
            &env,
            [
                ColorOverride {
                    slot: ColorSlot::Background,
                    rgb: 0,
                },
                ColorOverride {
                    slot: ColorSlot::Surface,
                    rgb: 0x111111,
                },
                ColorOverride {
                    slot: ColorSlot::Text,
                    rgb: MAX_RGB,
                },
                ColorOverride {
                    slot: ColorSlot::Primary,
                    rgb: 0x2563EB,
                },
                ColorOverride {
                    slot: ColorSlot::Accent,
                    rgb: 0x38BDF8,
                },
                ColorOverride {
                    slot: ColorSlot::Border,
                    rgb: 0x334155,
                },
            ],
        );
        assert_eq!(validate_theme_preferences(&env, &p), Ok(()));
    }
}
