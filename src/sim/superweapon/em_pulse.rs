//! Ares `Type=EMPulse`: the house's cannon buildings fire at the target cell
//! or, with `EMPulse.TargetSelf=yes`, detonate their primary weapon on
//! themselves. Mental Omega's Iron Guard, Crane, Time Freeze and Faux
//! Dominator are self-targeting.
//!
//! Source: Ares 3.0 documentation, Type=EMPulse
//! (https://ares-developers.github.io/Ares-docs/new/superweapons/types/empulse.html).
//! Ares 3.0 is closed source; this follows the documented behavior:
//! - the cannons are the house's buildings of an `EMPulse.Cannons=` type, or
//!   of any `EMPulseCannon=yes` type when the list is empty, with a primary
//!   weapon;
//! - a cannon fires only within `SW.RangeMinimum=`/`SW.RangeMaximum=` of the
//!   target (a negative bound uses its primary weapon's own range);
//! - at most `SW.MaxCount=` fire, a negative count meaning all;
//! - a self-targeting cannon does not actually fire: one bullet of its primary
//!   weapon is placed on it and detonated once (Ares
//!   `WeaponTypeExt::DetonateAt`), through the ordinary detonation
//!   transaction, so warhead extensions such as `IronCurtain.Duration=`
//!   apply.
//!
//! RESIDUALS:
//! - `EMPulse.TargetSelf=no` (turret rotation and a thrown bullet, or direct
//!   fire for an `EMPulseCannon=no` cannon) is not ported. Trigger: Mental
//!   Omega's nine target-firing EMPulse supers (Mercury, Judgement,
//!   Wallbuster and others). Effect: Launch declines and nothing fires.
//! - Cannon eligibility uses the launch-site predicate (owned, alive, on the
//!   map, operational) and the house's base building order; Ares's exact
//!   predicate and order are closed. Effect: with a limited `SW.MaxCount=`,
//!   which in-range cannons fire may differ.
//! - The optional pulse ball animation is not ported.

use crate::rules::overlay_types::OverlayTypeRegistry;
use crate::rules::ruleset::RuleSet;
use crate::rules::superweapon_targeting::LaunchSite;
use crate::rules::superweapon_type::SuperWeaponType;
use crate::sim::intern::InternedId;
use crate::sim::projectile::{
    ProjectileCoord, ProjectileDetonation, ProjectileDetonationReason, ProjectilePayload,
    ProjectileTarget,
};
use crate::sim::world::{FrameEffects, Simulation};

#[cfg(test)]
#[path = "em_pulse_tests.rs"]
mod tests;

/// The cannons that fire at `cell`, in the house's base building order.
fn firing_cannons(
    sim: &Simulation,
    rules: &RuleSet,
    owner: InternedId,
    sw: &SuperWeaponType,
    cell: (u16, u16),
) -> Vec<u64> {
    let (Some(settings), Some(policy), Some(house)) = (
        sw.em_pulse.as_ref(),
        rules.super_weapon_target_policy(&sw.id),
        sim.houses.get(&owner),
    ) else {
        return Vec::new();
    };
    let (minimum, maximum) = policy.range();
    let target = (i32::from(cell.0), i32::from(cell.1));
    let limit = usize::try_from(settings.max_count).unwrap_or(usize::MAX);
    let mut cannons = Vec::new();
    for &id in house.base_projection().buildings() {
        if cannons.len() >= limit {
            break;
        }
        let Some(entity) = sim.substrate.entities.get(id) else {
            continue;
        };
        let Some(object) = sim.interner.object_type(entity.type_ref(), rules) else {
            continue;
        };
        let listed = if settings.cannons.is_empty() {
            object.emp_pulse_cannon
        } else {
            settings
                .cannons
                .iter()
                .any(|name| name.eq_ignore_ascii_case(&object.id))
        };
        let Some(weapon) = object.primary().and_then(|name| rules.weapon(name)) else {
            continue;
        };
        if !listed {
            continue;
        }
        let coord = crate::sim::movement::ground_pose::object_get_coords(
            entity,
            sim.resolved_terrain.as_ref(),
        );
        let site = LaunchSite {
            center: (coord.x / 256, coord.y / 256),
            eligible: entity.owner() == owner
                && entity.is_object_alive()
                && !entity.lifecycle.in_limbo()
                && sim.building_operational_state(id, rules) == Some(true),
            range_minimum_override: (minimum < 0.0)
                .then(|| f64::from(weapon.minimum_range_leptons) / 256.0),
            range_maximum_override: (maximum < 0.0)
                .then(|| f64::from(weapon.range_leptons) / 256.0),
        };
        if site.admits(target, (minimum, maximum)) {
            cannons.push(id);
        }
    }
    cannons
}

/// Ares `WeaponTypeExt::DetonateAt(primary, cannon, cannon)`: one bullet of
/// the cannon's primary weapon at its full damage, placed on the cannon.
fn placed_detonation(
    sim: &mut Simulation,
    rules: &RuleSet,
    id: u64,
) -> Option<ProjectileDetonation> {
    let entity = sim.substrate.entities.get(id)?;
    let weapon = sim
        .interner
        .object_type(entity.type_ref(), rules)?
        .primary()
        .and_then(|name| rules.weapon(name))?;
    let coord =
        crate::sim::movement::ground_pose::object_get_coords(entity, sim.resolved_terrain.as_ref());
    let warhead = sim.interner.intern(weapon.warhead.as_deref()?);
    let projectile = weapon
        .projectile
        .as_deref()
        .map(|name| sim.interner.intern(name));
    let weapon_ref = sim.interner.intern(&weapon.id);
    Some(ProjectileDetonation {
        projectile_id: id,
        source_id: id,
        target: ProjectileTarget::Entity(id),
        impact: ProjectileCoord::new(coord.x, coord.y, coord.z),
        payload: ProjectilePayload::new(weapon.damage, warhead, projectile, weapon.bright)
            .with_installed_weapon(Some(weapon_ref)),
        reason: ProjectileDetonationReason::PlacedBullet,
    })
}

/// Ares `SW_EMPulse::Activate`; whether any cannon fired.
pub(super) fn launch(
    sim: &mut Simulation,
    rules: &RuleSet,
    owner: InternedId,
    sw: &SuperWeaponType,
    cell: (u16, u16),
    overlay_registry: Option<&OverlayTypeRegistry>,
    frame_effects: FrameEffects<'_>,
) -> bool {
    if !sw
        .em_pulse
        .as_ref()
        .is_some_and(|settings| settings.target_self)
    {
        log::debug!("Ares EMPulse {} fires at a target: not ported", sw.id);
        return false;
    }
    let cannons = firing_cannons(sim, rules, owner, sw, cell);
    let detonations: Vec<_> = cannons
        .into_iter()
        .filter_map(|id| placed_detonation(sim, rules, id))
        .collect();
    if detonations.is_empty() {
        return false;
    }
    sim.commit_logic_projectile_detonations(rules, overlay_registry, &detonations, frame_effects);
    true
}
