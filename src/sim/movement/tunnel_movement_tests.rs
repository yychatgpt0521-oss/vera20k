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
