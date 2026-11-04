//! Tunnel locomotor regressions through the Unit setter and object turns.
use super::*;
use crate::rules::locomotor_type::LocomotorKind;
use crate::sim::components::NavTargetRef;

/// MO's `DRIL` movement keys: the Tunnel CLSID and the misspelt retail
/// `Subterannean` movement zone.
const DRILLER: &str = "[VehicleTypes]\n0=DRIL\n[DRIL]\nStrength=240\nSpeed=4\nROT=4\n\
    SpeedType=Track\nMovementZone=Subterannean\n\
    Locomotor={4A582743-9839-11d1-B709-00A024DDAFD1}\n";

fn driller(start: (u16, u16)) -> (Simulation, RuleSet, OverlayTypeRegistry, u64) {
    let (mut sim, rules, registry) =
        crate::sim::world::entry_test_fixture::fixture_with_rules(DRILLER);
    let id = sim
        .spawn_object("DRIL", "Americans", start.0, start.1, 0, &rules)
        .unwrap();
    (sim, rules, registry, id)
}

fn order(sim: &mut Simulation, id: u64, cell: (u16, u16), rules: &RuleSet) {
    assert!(sim.set_unit_destination(
        id,
        NavTargetRef::cell(cell.0, cell.1),
        rules,
        true,
        FrameEffects::default(),
    ));
}

fn tunnel_state(sim: &Simulation, id: u64) -> Option<TunnelState> {
    sim.substrate
        .entities
        .get(id)
        .and_then(tunnel_runtime)
        .map(TunnelRuntime::state)
}

/// A trip of twelve cells tunnels (`0x0074608F`): the unit turns, digs, sinks
/// to -256 and leaves its cell (Mark(UP)), travels underground in the
/// Underground layer, and rises unmarked-to-marked at the destination's
/// centre before it idles with its NavCom cleared (`0x004DF0D0`).
#[test]
fn a_long_order_digs_travels_underground_and_surfaces_at_its_cell() {
    let (mut sim, rules, registry, id) = driller((15, 15));
    let goal = (15, 27);
    order(&mut sim, id, goal, &rules);
    assert_eq!(
        sim.substrate
            .entities
            .get(id)
            .unwrap()
            .locomotor
            .as_ref()
            .unwrap()
            .active_kind(),
        LocomotorKind::Tunnel,
        "the Tunnel arm keeps a trip of twelve cells"
    );
    let grid = sim.path_grid.clone();
    let mut seen = Vec::new();
    let mut burrowed_unmarked = false;
    for _ in 0..1500 {
        sim.advance_tick(&[], Some(&rules), grid.as_deref(), Some(&registry), 67);
        let state = tunnel_state(&sim, id).unwrap();
        if seen.last() != Some(&state) {
            seen.push(state);
        }
        let entity = sim.substrate.entities.get(id).unwrap();
        if state == TunnelState::Underground {
            burrowed_unmarked |= !entity.lifecycle.cell_marked()
                && sim.tunnel_location(id).unwrap().z == TUNNEL_DEPTH
                && crate::sim::world::entity_layer(entity, sim.resolved_terrain.as_ref(), None)
                    == crate::sim::world::display_layers::DisplayLayer::UNDERGROUND;
        }
        if state == TunnelState::Idle && seen.len() > 1 {
            break;
        }
    }
    use TunnelState::*;
    assert_eq!(
        seen,
        [
            Turning,
            Digging,
            Sinking,
            Underground,
            Rising,
            Surfaced,
            Idle
        ]
    );
    assert!(burrowed_unmarked, "a burrowed unit is off its cell at -256");
    let entity = sim.substrate.entities.get(id).unwrap();
    let location = sim.tunnel_location(id).unwrap();
    assert_eq!(
        (location.x, location.y),
        (goal.0 as i32 * 256 + 128, goal.1 as i32 * 256 + 128)
    );
    assert!(entity.lifecycle.cell_marked());
    assert!(entity.navigation.nav_com().is_none());
}

/// A neighbouring cell is not worth a tunnel (`0x00746080`): the Unit
/// setter's Tunnel arm installs a Drive over the idle Tunnel.
#[test]
fn a_one_cell_order_drives_over_the_tunnel() {
    let (mut sim, rules, _registry, id) = driller((15, 15));
    order(&mut sim, id, (16, 15), &rules);
    let locomotor = sim
        .substrate
        .entities
        .get(id)
        .unwrap()
        .locomotor
        .as_ref()
        .unwrap();
    assert_eq!(locomotor.active_kind(), LocomotorKind::Drive);
    assert_eq!(locomotor.effective_kind(), LocomotorKind::Tunnel);
    assert_eq!(tunnel_state(&sim, id), None);
}

/// The burrow test (`0x00486FF0`) refuses a slope (`+0x11C`) and a bridge
/// cell (`+0x140 & 0x500`), and a cell holding a Building.
#[test]
fn the_burrow_test_refuses_slopes_bridges_and_buildings() {
    let (mut sim, _rules, _registry, _id) = driller((15, 15));
    assert!(sim.cell_burrowable((16, 16)));
    let terrain = sim.resolved_terrain.as_mut().unwrap();
    terrain.cell_mut_for_test(16, 16).unwrap().slope_type = 1;
    terrain
        .cell_mut_for_test(17, 16)
        .unwrap()
        .bridge_facts
        .raw_flags |= 0x400;
    assert!(!sim.cell_burrowable((16, 16)));
    assert!(!sim.cell_burrowable((17, 16)));
    sim.substrate.occupancy.add(
        15,
        16,
        99,
        MovementLayer::Ground,
        None,
        crate::sim::occupancy::CellListInsertion::AppendBuilding,
    );
    assert!(!sim.cell_burrowable((15, 16)));
}

/// An order onto a building's cell, which refuses burrowing, runs the route
/// planner (`0x007447B0`, case `0x00745590`): the unit digs for the
/// burrowable cell nearest the building in its zone, and the building waits
/// in the NavQueue for the drive that follows.
#[test]
fn an_order_onto_a_building_digs_beside_it_and_queues_the_building() {
    let (mut sim, rules, _registry, id) = driller((15, 15));
    let hut = sim
        .spawn_object("CABHUT", "Americans", 15, 27, 0, &rules)
        .unwrap();
    assert!(!sim.cell_burrowable((15, 27)));
    assert!(sim.set_unit_destination(
        id,
        NavTargetRef::object(hut),
        &rules,
        true,
        FrameEffects::default(),
    ));
    let entity = sim.substrate.entities.get(id).unwrap();
    let Some(NavTargetRef::Cell { rx, ry }) = entity.navigation.nav_com() else {
        panic!(
            "the planner's first leg is a cell: {:?}",
            entity.navigation.nav_com()
        );
    };
    assert!((rx, ry) != (15, 27) && sim.cell_burrowable((rx as i16, ry as i16)));
    assert!(
        rx.abs_diff(15) <= 1 && ry.abs_diff(27) <= 1,
        "beside the hut: {rx},{ry}"
    );
    assert_eq!(entity.navigation.nav_queue(), [NavTargetRef::object(hut)]);
}

/// A new order to a driving Subterranean unit (`0x00741E8E`) waits at the
/// front of its navigation list while the Drive finishes the cell it is
/// heading for.
#[test]
fn a_new_order_to_a_driving_driller_waits_on_its_navigation_list() {
    let (mut sim, rules, registry, id) = driller((15, 15));
    order(&mut sim, id, (16, 15), &rules);
    let grid = sim.path_grid.clone();
    sim.advance_tick(&[], Some(&rules), grid.as_deref(), Some(&registry), 67);
    order(&mut sim, id, (15, 27), &rules);
    let entity = sim.substrate.entities.get(id).unwrap();
    assert_eq!(
        entity.locomotor.as_ref().unwrap().active_kind(),
        LocomotorKind::Drive
    );
    assert_eq!(entity.navigation.nav_list(), [NavTargetRef::cell(15, 27)]);
    assert!(entity.navigation.nav_com() != Some(NavTargetRef::cell(15, 27)));
}

/// While it surfaces (`Is_Surfacing @ 0x0072A1E0`) the unit holds its own
/// cell and keeps the order at the NavQueue's front (`0x007422F7`).
#[test]
fn a_surfacing_driller_holds_its_cell_and_queues_the_order() {
    let (mut sim, rules, _registry, id) = driller((15, 15));
    sim.substrate
        .entities
        .get_mut(id)
        .unwrap()
        .locomotor
        .as_mut()
        .unwrap()
        .tunnel_runtime_mut()
        .unwrap()
        .set_state_for_test(TunnelState::Rising);
    order(&mut sim, id, (15, 27), &rules);
    let entity = sim.substrate.entities.get(id).unwrap();
    assert_eq!(
        entity.navigation.nav_com(),
        Some(NavTargetRef::cell(15, 15))
    );
    assert_eq!(entity.navigation.nav_queue(), [NavTargetRef::cell(15, 27)]);
}
