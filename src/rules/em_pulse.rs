//! Ares `Type=EMPulse` configuration. The ordered Rules owner resolves the
//! cannon list; this module holds immutable configuration, not runtime state.
//!
//! Source: Ares 3.0 documentation, Type=EMPulse
//! (https://ares-developers.github.io/Ares-docs/new/superweapons/types/empulse.html).
//! Ares 3.0 is closed source; the readers follow the documented keys and
//! defaults. `SW.RangeMinimum=`/`SW.RangeMaximum=` belong to the shared
//! targeting policy.

/// `EMPulse.Cannons=`, `EMPulse.TargetSelf=` and `SW.MaxCount=`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct EmPulseSettings {
    /// BuildingTypes that may fire. Empty: every `EMPulseCannon=yes` type.
    pub cannons: Vec<String>,
    /// Each firing cannon detonates its primary weapon on itself.
    pub target_self: bool,
    /// Most cannons that fire; negative is unlimited. Ares default 1.
    pub max_count: i32,
}

impl Default for EmPulseSettings {
    fn default() -> Self {
        Self {
            cannons: Vec::new(),
            target_self: false,
            max_count: 1,
        }
    }
}
