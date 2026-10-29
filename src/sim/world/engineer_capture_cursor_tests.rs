//! Ares Multi Engineer: Mental Omega's EngineerCaptureLevel=35% decides the
//! engineer's action on a hostile building only with the session's
//! MultiEngineer option, which a campaign never sets.
use super::*;
use crate::rules::ini_parser::IniFile;
use crate::sim::house_state::HouseState;

fn scene() -> (Simulation, RuleSet, u64, u64) {
    let ini = crate::sim::arena_fixture::clear_ground_ini(
        "[General]\nEngineerCaptureLevel=35%\n\
         [Countries]\n0=Americans\n1=Russians\n[InfantryTypes]\n0=ENGINEER\n\
         [BuildingTypes]\n0=FACILITY\n\
         [ENGINEER]\nImage=GI\nStrength=75\nEngineer=yes\nSpeed=4\n\
         Locomotor={4A582744-9839-11D1-B709-00A024DDAFD1}\n\
         [FACILITY]\nStrength=1000\nCapturable=yes\nRepairable=yes\n",
    );
    let art = IniFile::from_str(&format!(
        "{}\n[FACILITY]\nFoundation=1x1\n",
        crate::rules::retail_ini_fixture::GI_ART_EXCERPT
    ));
    let rules = RuleSet::from_ini_with_fixed_art_for_test(&ini, &art).unwrap();
    let mut sim = Simulation::with_seed(7);
    crate::sim::arena_fixture::flat_ground(&mut sim, &rules);
    sim.resolve_type_handles(&rules);
    for (name, human) in [("Americans", true), ("Russians", false)] {
        let id = sim.intern(name);
        sim.houses
            .register(HouseState::new(id, 0, None, human, 10_000, 10));
    }
    let engineer = sim
        .spawn_object("ENGINEER", "Americans", 10, 12, 0, &rules)
        .unwrap();
    let building = sim
        .spawn_object("FACILITY", "Russians", 16, 12, 0, &rules)
        .unwrap();
    (sim, rules, engineer, building)
}

#[test]
fn healthy_hostile_building_is_captured_unless_multi_engineer_is_on() {
    let (mut sim, rules, engineer, building) = scene();
    assert_eq!(
        sim.engineer_building_action(engineer, building, &rules),
        Some(EngineerBuildingAction::Capture),
        "a campaign never applies EngineerCaptureLevel"
    );
    sim.session
        .set_game_mode(crate::sim::scenario_session::ScenarioGameMode::from_legacy_nonzero(true));
    sim.session.game_options_mut_for_test().multi_engineer = true;
    // A nonzero session mode compares the engineer's House to LocalPlayer.
    let player = sim.substrate.entities.get(engineer).unwrap().owner();
    sim.session.set_current_house_for_test(Some(player));
    assert_eq!(
        sim.engineer_building_action(engineer, building, &rules),
        Some(EngineerBuildingAction::Damage)
    );
}
