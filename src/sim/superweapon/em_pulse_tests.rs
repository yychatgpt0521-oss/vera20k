//! Mental Omega's Iron Guard shape: a self-targeting, auto-firing Ares
//! EMPulse whose cannon's Iron Curtain warhead protects nearby allies.
use super::*;
use crate::rules::ini_parser::IniFile;
use crate::rules::native_processing::RulesLayerStack;
use crate::sim::superweapon::invulnerability::is_invulnerable;
use crate::sim::superweapon::{SuperWeaponInstance, superweapon_views_for_owner};

fn scene(extra: &str) -> (RuleSet, Simulation, InternedId, [u64; 3]) {
    let mut ini = crate::sim::arena_fixture::clear_ground_ini(
        "[Countries]\n0=Americans\n1=Russians\n[InfantryTypes]\n0=GI\n\
         [GI]\nStrength=100\nArmor=none\n[VehicleTypes]\n0=TANK\n\
         [TANK]\nStrength=300\nArmor=light\n[BuildingTypes]\n0=GUARD\n\
         [GUARD]\nStrength=700\nEMPulseCannon=yes\nPrimary=GuardWeapon\n\
         SuperWeapon=GuardSpecial\n[SuperWeaponTypes]\n0=GuardSpecial\n\
         [GuardSpecial]\nType=EMPulse\nIsPowered=no\nRechargeTime=1\n\
         SW.AutoFire=yes\nSW.ManualFire=no\nSW.AITargeting=Self\n\
         EMPulse.TargetSelf=yes\nSW.MaxCount=-1\nSW.RangeMaximum=5\nSW.ShowCameo=no\n\
         [WeaponTypes]\n0=GuardWeapon\n[GuardWeapon]\nDamage=-1\nWarhead=GuardWH\n\
         [Warheads]\n0=GuardWH\n[GuardWH]\nCellSpread=3\nAffectsEnemies=no\n\
         IronCurtain.Duration=125\nVerses=0%,0%,0%,3%,3%,3%,0%,0%,0%,3%,0%\n",
    );
    ini.merge(&IniFile::from_str(extra));
    let processed = RulesLayerStack::new(ini)
        .process_with_fixed_art(&IniFile::from_str("[GUARD]\nFoundation=1x1\n"))
        .unwrap();
    let rules = RuleSet::from_processed_rules(&processed).unwrap();
    let mut sim = crate::sim::arena_fixture::rival_arena(&rules, 17);
    sim.intern_rule_type_ids(&rules);
    sim.resolve_type_handles(&rules);
    let owner = sim.interner.get("Americans").unwrap();
    sim.spawn_object_at_height("GUARD", "Americans", 12, 15, 0, 0, &rules)
        .unwrap();
    let ally = sim
        .spawn_object_at_height("TANK", "Americans", 14, 15, 0, 0, &rules)
        .unwrap();
    let enemy = sim
        .spawn_object_at_height("TANK", "Russians", 12, 17, 0, 0, &rules)
        .unwrap();
    let infantry = sim
        .spawn_object_at_height("GI", "Americans", 13, 14, 0, 0, &rules)
        .unwrap();
    let sw = sim.interner.get("GuardSpecial").unwrap();
    let mut instance = SuperWeaponInstance::new(sw, owner, 0);
    instance.is_active = true;
    instance.is_ready = true;
    sim.super_weapons
        .map_mut_for_test()
        .entry(owner)
        .or_default()
        .insert(sw, instance);
    (rules, sim, owner, [ally, enemy, infantry])
}

/// The cannon's placed bullet curtains allies whose Verses is not 0%, never
/// enemies, and spares 0% infantry (whom a curtain would kill).
#[test]
fn self_targeting_em_pulse_curtains_allies_through_its_cannon_warhead() {
    let (rules, mut sim, owner, [ally, enemy, infantry]) = scene("");
    crate::sim::house_strategy::update_strategy(
        &mut sim,
        &rules,
        owner,
        None,
        FrameEffects::default(),
    );
    let frame = sim.session.binary_frame();
    let curtained = |id: u64| {
        is_invulnerable(
            sim.entities().get(id).unwrap().invulnerability.as_ref(),
            frame,
        )
    };
    assert!(curtained(ally), "allied tank within CellSpread");
    assert!(!curtained(enemy), "AffectsEnemies=no");
    assert!(!curtained(infantry), "0% Verses");
    assert!(sim.entities().get(infantry).unwrap().is_object_alive());
    assert_eq!(
        sim.entities()
            .get(ally)
            .unwrap()
            .invulnerability
            .as_ref()
            .unwrap()
            .timer()
            .remaining(frame as i32),
        125
    );
}

/// Ares `SW.ShowCameo=no` hides only an auto-firing super's cameo.
#[test]
fn show_cameo_no_hides_only_an_auto_firing_super() {
    for (extra, shown) in [("", false), ("[GuardSpecial]\nSW.AutoFire=no\n", true)] {
        let (rules, sim, owner, _) = scene(extra);
        let views = superweapon_views_for_owner(&sim, &rules, &owner);
        assert_eq!(views[0].ares_show_cameo, shown, "{extra:?}");
    }
}
