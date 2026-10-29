//! Ares per-weapon Ivan Bomb settings (`IvanBomb.*` on a weapon whose warhead
//! has `IvanBomb=yes`). Unset values fall back to the stock `[CombatDamage]`
//! and `[AudioVisual]` keys where the bomb is used.
//!
//! Source: Ares 3.0 documentation, Ivan Bombs
//! (https://ares-developers.github.io/Ares-docs/new/weapons/ivanbombs.html)
//! and the migration guide for FlickerRate/AttachSound/TickingSound. Ares 3.0
//! is closed source; the readers follow the documented keys and defaults.

use crate::rules::ini_parser::IniSection;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct IvanBombRules {
    /// `IvanBomb.Delay=`: frames until a time bomb goes off.
    pub delay: Option<i32>,
    /// `IvanBomb.Warhead=`.
    pub warhead: Option<String>,
    /// `IvanBomb.Damage=`.
    pub damage: Option<i32>,
    /// `IvanBomb.Detachable=` (default yes): Engineers may remove it.
    pub detachable: bool,
    /// `IvanBomb.DestroysBridges=` (default yes): a bombed Bridge Repair Hut
    /// collapses its bridge.
    pub destroys_bridges: bool,
    /// `IvanBomb.Image=`: the overlay SHP instead of BOMBCURS.
    pub image: Option<String>,
    /// `IvanBomb.FlickerRate=`.
    pub flicker_rate: Option<i32>,
    /// `IvanBomb.AttachSound=`.
    pub attach_sound: Option<String>,
    /// `IvanBomb.TickingSound=`.
    pub ticking_sound: Option<String>,
}

impl Default for IvanBombRules {
    fn default() -> Self {
        Self {
            delay: None,
            warhead: None,
            damage: None,
            detachable: true,
            destroys_bridges: true,
            image: None,
            flicker_rate: None,
            attach_sound: None,
            ticking_sound: None,
        }
    }
}

impl IvanBombRules {
    pub fn from_section(section: &IniSection) -> Self {
        let name = |key| section.read_name(key, 0x800).map(str::to_owned);
        let int = |key| {
            section
                .read_name(key, 0x800)
                .map(|_| section.read_ares_int(key, 0))
        };
        Self {
            delay: int("IvanBomb.Delay"),
            warhead: name("IvanBomb.Warhead"),
            damage: int("IvanBomb.Damage"),
            detachable: section.read_ares_bool("IvanBomb.Detachable", true),
            destroys_bridges: section.read_ares_bool("IvanBomb.DestroysBridges", true),
            image: name("IvanBomb.Image"),
            flicker_rate: int("IvanBomb.FlickerRate"),
            attach_sound: name("IvanBomb.AttachSound"),
            ticking_sound: name("IvanBomb.TickingSound"),
        }
    }
}
