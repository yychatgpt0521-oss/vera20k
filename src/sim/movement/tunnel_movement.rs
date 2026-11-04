//! Tunnel locomotor: `TunnelLocomotionClass` (ILocomotion vtable
//! `0x007F5A24`, constructor `0x00728A00`, CLSID `0x007E9A50`).
//!
//! A tunnel unit turns to face its destination, waits a dig delay, sinks to
//! Location Z -256, travels there in a straight line 19 leptons a frame, rises
//! at the destination and waits again before it idles. No retail YR type
//! selects it; mods do (Mental Omega's `DRIL`, `SMIN`, `SCAV`). The Unit
//! setter decides per order whether a surface tunnel unit digs or drives a
//! Drive piggyback instead ([`Simulation::unit_tunnel_arm`]).
//!
//! The class state (`Tunnel+0x18`) is one of eight values ([`TunnelState`]);
//! its other fields are the destination (`+0x1C`) and the dig delay (the
//! timer at `+0x28` with its rate at `+0x34`). The byte at `+0x38` has no
//! reader: Process and the arrival clear it and `Z_Adjust` sets it.
//!
//! RESIDUALS:
//! - The Unit setter's two Subterranean arms (`0x00741E8E..0x00742088`, a
//!   moving Drive's nav-list insert; `0x0074208A..0x007423CD`, the route
//!   planner `0x007447B0` and the NavQueue hold of an order given while the
//!   unit surfaces) are not ported. Trigger: a second order to a driving or
//!   surfacing tunnel unit, or a destination the planner would move. Effect:
//!   the driving unit re-paths at once; the surfacing unit drops the order
//!   when it idles; the order keeps its own cell. Frequency: every such order.
//! - The class `Can_Enter_Cell` (`0x0072A090`) answers only through
//!   [`cell_allows_burrowing`] for the Unit setter and FNPC; the `+0x1C` query
//!   Foot `0x004D9C10` forwards for a locomotor-checking mover (docking,
//!   reinforcement and teleport scans) still reads the constant zero.
//! - EMP and Unit+6D8 (Unit vt+0x37C, `0x00746C90`) do not gate Move_To, as
//!   for Drive.
//! - The display layer vectors keep a burrowed unit in Ground; its layer
//!   query answers Underground (`0x0072A1A0`) for Mark and the callers.
//!
//! ## Dependency rules
//! - Part of sim/ — sim/ NEVER depends on render/, ui/, audio/, net/.

use crate::map::cell_index::NativeCellIdentity;
use crate::map::resolved_terrain::NativeCellQuery;
use crate::rules::locomotor_type::{MovementZone, SpeedType};
use crate::rules::overlay_types::OverlayTypeRegistry;
use crate::rules::ruleset::RuleSet;
use crate::sim::cell_rect::PlayfieldBounds;
use crate::sim::components::{AnimClassSpawnDescriptor, DriveCoord};
use crate::sim::game_entity::GameEntity;
use crate::sim::occupancy::OccupancyGrid;
use crate::sim::timer::CdTimer;
use crate::sim::world::{FrameAdvanceError, FrameEffects, Simulation};
use crate::util::native_x87::{MaskedX87Chop53 as X, NativeF64Bits};

use super::foot_coordinate::NULL_COORD;
use super::locomotor::MovementLayer;

/// The Location Z a burrowed unit travels at (`0x00728F89`, `0x007295E6`).
pub const TUNNEL_DEPTH: i32 = -256;
/// Leptons a burrowed unit moves each frame (`[0x007F5B20]`, 19.0).
const UNDERGROUND_STEP: i32 = 19;
/// An underground leg ends within this many leptons of its destination
/// (`0x007295C5`).
const ARRIVAL_DISTANCE: i32 = 20;
/// The slowest vertical dig step (`0x00728FE8`, `0x0072995F`).
const MIN_DIG_STEP: i32 = 5;
/// GetHeight the dig sound and animation mark when a unit breaks the surface
/// (`0x007297BC`, `0x0072998B`, `0x007299A0`).
const SURFACE_HEIGHT: i32 = -50;
/// `[0x007F5B18]`: the dig delays are this divided by the type's `ROT=`.
const DIG_DELAY_NUMERATOR: NativeF64Bits = NativeF64Bits::from_bits(0x4050_0000_0000_0000);
/// The AnimClass flags of the Dig animation (`0x00728F3E`, `0x0072932F`).
const DIG_ANIM_DRAW_FLAGS: u32 = 0x600;

/// `Tunnel+0x18`, in native value order (0..7).
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize,
)]
pub enum TunnelState {
    /// 0: no movement; Process returns before its tail.
    #[default]
    Idle,
    /// 1: turning to face the destination (`0x007291F0`).
    Turning,
    /// 2: the dig-in delay on the surface.
    Digging,
    /// 3: sinking to [`TUNNEL_DEPTH`].
    Sinking,
    /// 4: burrowed, travelling (`0x00729580`).
    Underground,
    /// 5: rising to the ground (`0x007298F0`).
    Rising,
    /// 6: the delay after breaking the surface.
    Surfaced,
    /// 7: a dig stopped before it went under (Stop_Moving in state 2).
    Abandoned,
}

/// One Tunnel locomotor object's class state.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct TunnelRuntime {
    state: TunnelState,
    /// `+0x1C`: the destination; the zero coordinate is none.
    destination: DriveCoord,
    /// `+0x28` start and `+0x30` time left of the dig delay.
    dig_timer: CdTimer,
    /// `+0x34`: the delay's rate; zero counts as elapsed.
    dig_rate: i32,
}

impl TunnelRuntime {
    /// The constructor (`0x00728A00`): idle, no destination, a zero delay
    /// started at the current frame.
    pub(crate) fn constructed(binary_frame: u32) -> Self {
        Self {
            state: TunnelState::Idle,
            destination: NULL_COORD,
            dig_timer: CdTimer::started(binary_frame as i32, 0),
            dig_rate: 0,
        }
    }

    pub(crate) const fn state(&self) -> TunnelState {
        self.state
    }

    /// `Is_Moving @ 0x00728A50`.
    pub(crate) fn is_moving(&self) -> bool {
        self.state != TunnelState::Idle
    }

    /// `Is_Moving_Now @ 0x00728A60`: moving and past the turn.
    pub(crate) fn is_moving_now(&self) -> bool {
        !matches!(self.state, TunnelState::Idle | TunnelState::Turning)
    }

    /// `Destination @ 0x00728A90` while moving: the stored coordinate.
    pub(crate) const fn destination(&self) -> DriveCoord {
        self.destination
    }

    /// `In_Which_Layer @ 0x0072A1A0`: Underground in state 4, else Ground.
    pub(crate) fn burrowed(&self) -> bool {
        self.state == TunnelState::Underground
    }

    /// `Can_Fire @ 0x0072A1C0`: any state but idle answers FireError Moving.
    pub(crate) fn refuses_fire(&self) -> bool {
        self.state != TunnelState::Idle
    }

    /// `Is_Surfacing @ 0x0072A1E0`.
    pub(crate) fn is_surfacing(&self) -> bool {
        matches!(self.state, TunnelState::Rising | TunnelState::Surfaced)
    }

    /// `Visual_Character @ 0x007291D0`: burrowed, Hidden (5) for a sensor
    /// query and Ripple (4) for drawing; otherwise none, and Foot
    /// `0x004DA4E0` asks Techno's own.
    pub(crate) fn visual_character(&self, sensor: bool) -> Option<u8> {
        self.burrowed().then_some(if sensor { 5 } else { 4 })
    }

    /// `Is_To_Have_Shadow @ 0x0072A060`: on the surface (states 0, 1, 2, 6
    /// and 7).
    pub fn casts_shadow(&self) -> bool {
        matches!(
            self.state,
            TunnelState::Idle
                | TunnelState::Turning
                | TunnelState::Digging
                | TunnelState::Surfaced
                | TunnelState::Abandoned
        )
    }

    /// The hull pitch `Draw_Matrix @ 0x00729B40` rotates by, in radians:
    /// nose down while sinking, nose up while rising, and easing between the
    /// two through the delays, by the delay's progress.
    pub fn draw_pitch(&self, binary_frame: u32) -> f64 {
        const QUARTER: f64 = std::f64::consts::FRAC_PI_2;
        let progress = || self.delay_progress(binary_frame);
        match self.state {
            TunnelState::Digging => progress() * QUARTER,
            TunnelState::Sinking => QUARTER,
            TunnelState::Rising => -QUARTER,
            TunnelState::Surfaced => (1.0 - progress()) * -QUARTER,
            TunnelState::Abandoned => (1.0 - progress()) * QUARTER,
            TunnelState::Idle | TunnelState::Turning | TunnelState::Underground => 0.0,
        }
    }

    /// The delay's elapsed fraction, `(rate - left) / rate` (1 for a zero
    /// rate), which Draw_Matrix reads (`0x00729C9C..0x00729D9C`).
    fn delay_progress(&self, binary_frame: u32) -> f64 {
        if self.dig_rate == 0 {
            return 1.0;
        }
        let left = self.dig_timer.remaining(binary_frame as i32);
        f64::from(self.dig_rate.wrapping_sub(left)) / f64::from(self.dig_rate)
    }

    /// Process's delay test (`0x00728EA0..0x00728ECE` and its twins): a zero
    /// rate, or `(rate - left) / rate == 1.0`, which for whole frames is a
    /// zero time left.
    fn delay_elapsed(&self, binary_frame: u32) -> bool {
        self.dig_rate == 0 || self.dig_timer.remaining(binary_frame as i32) == 0
    }

    fn start_delay(&mut self, binary_frame: u32, rate: i32) {
        self.dig_timer.start(binary_frame as i32, rate);
        self.dig_rate = rate;
    }

    #[cfg(any(test, feature = "test-support"))]
    pub(crate) fn set_state_for_test(&mut self, state: TunnelState) {
        self.state = state;
    }

    #[cfg(any(test, feature = "test-support"))]
    pub(crate) fn set_destination_for_test(&mut self, destination: DriveCoord) {
        self.destination = destination;
    }
}

/// The cell's burrow test (`CellClass @ 0x00486FF0`), which the Tunnel
/// `Can_Enter_Cell` (`0x0072A0BF`), the Unit setter and FNPC's arg11
/// (`0x0056DE5F`) ask. A cell outside the playfield border
/// (`MapClass::Is_Cell_In_Playfield_CellClass(cell, 1) @ 0x00578540`)
/// answers yes. Otherwise it refuses a tile whose `AllowBurrowing=` is off
/// (`0x0048700A..0x0048702A`, an out-of-range tile skipping the test), a
/// slope (`+0x11C`), a bridge cell (`+0x140 & 0x500`) and, while a game runs
/// (`[0x00A8E9A0]`, which live logic holds), a cell holding a Building or a
/// TerrainClass object (What_Am_I 6 / 0x24 in its object list).
pub(crate) fn cell_allows_burrowing(
    cells: &NativeCellQuery<'_>,
    cell: NativeCellIdentity,
    bounds: Option<PlayfieldBounds>,
    occupancy: Option<&OccupancyGrid>,
) -> bool {
    if !crate::sim::cell_rect::retained_cell_is_in_playfield(cell, bounds, cells) {
        return true;
    }
    let terrain = cells.terrain();
    if !terrain.native_cell_tile_allows_burrowing(cell)
        || cells.ground_fields(cell).1 != 0
        || cells.flags(cell) & 0x500 != 0
    {
        return false;
    }
    let NativeCellIdentity::Real(index) = cell else {
        return true;
    };
    let resolved = &terrain.cells()[index];
    let building = occupancy.is_some_and(|occupancy| {
        occupancy
            .first_building_on_layer(resolved.rx, resolved.ry, MovementLayer::Ground)
            .is_some()
    });
    !building && resolved.terrain_object_occupation.is_none()
}

/// What one Tunnel Process did for the object turn.
#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct TunnelProcess {
    /// The surfacing delay ended with `Per_Cell_Process(2)`.
    pub(crate) per_cell_ran: bool,
    pub(crate) bridge_state_changed: bool,
}

fn tunnel_runtime(entity: &GameEntity) -> Option<&TunnelRuntime> {
    entity.locomotor.as_ref()?.tunnel_runtime()
}

/// `max(5, ftol(GetCurrentSpeed * TunnelSpeed))`, the vertical step
/// (`0x00728FCD..0x00728FF0`, `0x00729943..0x00729969`).
fn dig_step(current_speed: i32, tunnel_speed: NativeF64Bits) -> i32 {
    X::ftol_i32_low_masked(X::mul(
        X::load_i32(current_speed),
        X::load_f64(tunnel_speed),
    ))
    .max(MIN_DIG_STEP)
}

/// The dig delays: `ftol(64 / ROT / TunnelSpeed)` going under
/// (`0x0072929F..0x007292BA`) and `ftol(64 / ROT)` after surfacing
/// (`0x00729A4E..0x00729A60`).
fn dig_delay(rot: i32, tunnel_speed: Option<NativeF64Bits>) -> i32 {
    let delay = X::div(X::load_f64(DIG_DELAY_NUMERATOR), X::load_i32(rot));
    let delay = match tunnel_speed {
        Some(speed) => X::div(delay, X::load_f64(speed)),
        None => delay,
    };
    X::ftol_i32_low_masked(delay)
}

impl Simulation {
    fn tunnel_runtime_mut(&mut self, id: u64) -> Option<&mut TunnelRuntime> {
        self.substrate
            .entities
            .get_mut(id)?
            .locomotor
            .as_mut()?
            .tunnel_runtime_mut()
    }

    fn set_tunnel_state(&mut self, id: u64, state: TunnelState) {
        if let Some(runtime) = self.tunnel_runtime_mut(id) {
            runtime.state = state;
        }
    }

    fn tunnel_location(&self, id: u64) -> Option<DriveCoord> {
        let entity = self.substrate.entities.get(id)?;
        Some(super::ground_pose::object_location(
            entity,
            self.resolved_terrain.as_ref(),
        ))
    }

    fn tunnel_height(&self, id: u64) -> i32 {
        self.substrate.entities.get(id).map_or(0, |entity| {
            super::air_movement::current_fly_height(entity, self.resolved_terrain.as_ref())
        })
    }

    /// `MapClass::GetCellFloorHeight @ 0x00578080` at `point`.
    fn tunnel_floor(&self, point: DriveCoord) -> i32 {
        self.resolved_terrain
            .as_ref()
            .and_then(|terrain| {
                super::ground_pose::query_ground_height(&NativeCellQuery::canonical(terrain), point)
                    .ok()
            })
            .unwrap_or(0)
    }

    fn tunnel_current_speed(&self, id: u64, rules: &RuleSet) -> i32 {
        self.substrate.entities.get(id).map_or(0, |entity| {
            super::foot_speed::SpeedRules::new(rules, &self.interner, &self.houses)
                .owner_current_speed(entity)
        })
    }

    /// The burrow test of the cell under `point`.
    fn tunnel_burrowable_at(&self, point: DriveCoord) -> bool {
        self.burrowable(|cells| cells.lookup_world(point.x, point.y))
    }

    /// The burrow test of `cell`.
    pub(crate) fn cell_burrowable(&self, cell: (i16, i16)) -> bool {
        self.burrowable(|cells| cells.lookup(cell))
    }

    fn burrowable(&self, lookup: impl FnOnce(&NativeCellQuery<'_>) -> NativeCellIdentity) -> bool {
        let Some(terrain) = self.resolved_terrain.as_ref() else {
            return true;
        };
        let cells = NativeCellQuery::canonical(terrain);
        let cell = lookup(&cells);
        cell_allows_burrowing(
            &cells,
            cell,
            self.playfield_bounds(),
            Some(&self.substrate.occupancy),
        )
    }

    /// `VocClass::PlayAt(DigSound, Location)` (`0x007509E0`) and, with
    /// `anim`, `AnimClass(Dig, Location, 0, 1, 0x600, 0, 0)` (`0x00421EA0`).
    fn tunnel_dig_effects(&mut self, id: u64, rules: &RuleSet, anim: bool) {
        let Some(location) = self.tunnel_location(id) else {
            return;
        };
        let world = crate::sim::anim_class::AnimWorldCoord {
            x: location.x,
            y: location.y,
            z: location.z,
        };
        if let Some(sound) = rules.general.dig_sound.clone() {
            self.sound_events
                .emit(crate::sim::world::SimSoundEvent::voc_at_world_coord(
                    sound, world,
                ));
        }
        if !anim || rules.general.dig.name.is_empty() {
            return;
        }
        let (rx, ry, sub_x, sub_y, z) = world.to_cell_sub_z();
        let type_id = self.interner.intern(&rules.general.dig.name);
        let descriptor = AnimClassSpawnDescriptor {
            delay: 0,
            loop_count: 1,
            draw_flags: DIG_ANIM_DRAW_FLAGS,
            z_adjust: 0,
            reverse: false,
            ..AnimClassSpawnDescriptor::new(type_id, rx, ry, sub_x, sub_y, z)
        };
        if let Err(error) = self.spawn_anim_at_world(rules, descriptor, world) {
            log::debug!(
                "tunnel dig [{}] did not construct: {error}",
                rules.general.dig.name
            );
        }
    }

    /// `TunnelLocomotionClass::Move_To @ 0x00728AF0`. A paralysed or warping
    /// owner refuses (vt+0x380, vt+0x1D4, vt+0x1D8). The destination is the
    /// request with its Z raised to the floor (UnitType vt+0x6C,
    /// `0x00747EB0`). A non-null request to an owner below the ground that is
    /// not mid-dig (states 0, 4 and 7) takes Mark(UP) and goes underground
    /// (`0x00728BA8..0x00728BE0`); an idle one starts to turn.
    pub(crate) fn tunnel_move_to(
        &mut self,
        id: u64,
        coord: DriveCoord,
        rules: &RuleSet,
        registry: Option<&OverlayTypeRegistry>,
        effects: FrameEffects<'_>,
    ) -> bool {
        let frame = self.session.binary_frame();
        let Some(entity) = self.substrate.entities.get(id) else {
            return false;
        };
        if tunnel_runtime(entity).is_none()
            || entity.parasite.is_paralyzed(frame)
            || super::locomotor_owner::owner_is_warping(entity)
        {
            return false;
        }
        let destination = DriveCoord {
            z: coord.z.max(self.tunnel_floor(coord)),
            ..coord
        };
        let height = self.tunnel_height(id);
        let Some(runtime) = self.tunnel_runtime_mut(id) else {
            return false;
        };
        runtime.destination = destination;
        let state = runtime.state;
        let goes_under = coord != NULL_COORD
            && height < 0
            && matches!(
                state,
                TunnelState::Idle | TunnelState::Underground | TunnelState::Abandoned
            );
        if goes_under {
            self.foot_mark_remove(id, rules, registry, effects);
            self.set_tunnel_state(id, TunnelState::Underground);
        }
        if let Some(runtime) = self.tunnel_runtime_mut(id)
            && runtime.state == TunnelState::Idle
        {
            runtime.state = TunnelState::Turning;
        }
        true
    }

    /// `TunnelLocomotionClass::Stop_Moving @ 0x00728C00`. Turning drops the
    /// destination; a dig still on the surface is abandoned (state 7) and a
    /// sinking unit rises again (state 5), both dropping it too. A burrowed
    /// unit more than a cell diagonal from its destination surfaces at the
    /// nearest burrowable cell instead ([`Self::tunnel_surface_cell`]), or
    /// dies by `C4Warhead=` when there is none. The later states drop the
    /// destination.
    pub(crate) fn tunnel_stop_moving(
        &mut self,
        id: u64,
        rules: &RuleSet,
        registry: Option<&OverlayTypeRegistry>,
        effects: FrameEffects<'_>,
    ) {
        let Some(state) = self
            .substrate
            .entities
            .get(id)
            .and_then(tunnel_runtime)
            .map(TunnelRuntime::state)
        else {
            return;
        };
        let next = match state {
            TunnelState::Idle => return,
            TunnelState::Digging => TunnelState::Abandoned,
            TunnelState::Sinking => TunnelState::Rising,
            TunnelState::Underground => {
                self.tunnel_stop_underground(id, rules, registry, effects);
                return;
            }
            other => other,
        };
        if let Some(runtime) = self.tunnel_runtime_mut(id) {
            runtime.state = next;
            runtime.destination = NULL_COORD;
        }
    }

    /// Stop_Moving's state 4 (`0x00728C1E..0x00728DCD`).
    fn tunnel_stop_underground(
        &mut self,
        id: u64,
        rules: &RuleSet,
        registry: Option<&OverlayTypeRegistry>,
        effects: FrameEffects<'_>,
    ) {
        let (Some(location), Some(destination)) = (
            self.tunnel_location(id),
            self.substrate
                .entities
                .get(id)
                .and_then(tunnel_runtime)
                .map(TunnelRuntime::destination),
        ) else {
            return;
        };
        // 0x00728C64..0x00728C84: Sqrt_Approx/ftol of the planar distance,
        // against the static Sqrt_Approx(2 * 256^2) (0x007287F0).
        let distance = crate::util::native_x87::sqrt_approx_length([
            location.x.wrapping_sub(destination.x),
            location.y.wrapping_sub(destination.y),
        ]);
        let diagonal = crate::util::native_x87::sqrt_approx_length([256, 256]);
        if distance <= diagonal {
            return;
        }
        match self.tunnel_surface_cell(id, MovementZone::Normal) {
            Some((x, y)) => {
                if let Some(runtime) = self.tunnel_runtime_mut(id) {
                    runtime.destination = DriveCoord::cell(x as u16, y as u16, 0);
                }
            }
            None => {
                self.receive_own_health_c4(id, rules, registry, effects);
                if let Some(runtime) = self.tunnel_runtime_mut(id) {
                    runtime.destination = NULL_COORD;
                }
            }
        }
    }

    /// The burrowable cell nearest the owner's cell that both Stop_Moving
    /// (`0x00728C8A..0x00728D48`) and the underground arrival
    /// (`0x00729668..0x0072972D`) search for: FNPC (`0x0056DC20`) with Track
    /// speed, the cell's Normal zone (`0x0056D230`), MovementZone Normal, a
    /// one-cell footprint, bridges allowed and arg11 requiring the burrow
    /// test; then, for a NullCell answer, the same with no zone and
    /// `retry_zone` as the MovementZone (Normal from Stop_Moving,
    /// Subterranean from the arrival).
    fn tunnel_surface_cell(&self, id: u64, retry_zone: MovementZone) -> Option<(i16, i16)> {
        use crate::sim::find_nearby_cell::{
            NearbyArgs, NearbyFootprint, NearbySearchOptions, PassabilityArgs,
        };
        let entity = self.substrate.entities.get(id)?;
        let seed = super::foot_path::coord_cell(super::ground_pose::position_world_coord(
            &entity.position,
        ));
        let zone = self
            .resolved_terrain
            .as_ref()
            .zip(self.zone_grid.as_ref())
            .and_then(|(terrain, zones)| {
                zones.get_zone_id_native(
                    terrain,
                    (seed.0 as u16, seed.1 as u16),
                    MovementZone::Normal,
                    false,
                )
            });
        let search = |required_zone_id, movement_zone| {
            self.nearby_passable_cell(
                (i32::from(seed.0), i32::from(seed.1)),
                NearbyArgs {
                    options: NearbySearchOptions {
                        require_burrowable: true,
                        ..Default::default()
                    },
                    ..NearbyArgs::plain(
                        PassabilityArgs {
                            speed_type: SpeedType::Track,
                            required_zone_id,
                            movement_zone,
                            bridge_aware_zone: false,
                        },
                        NearbyFootprint::SINGLE,
                    )
                },
            )
            .filter(|cell| *cell != (0, 0))
        };
        search(zone, MovementZone::Normal)
            .or_else(|| search(None, retry_zone))
            .map(|(x, y)| (x as i16, y as i16))
    }

    /// `TunnelLocomotionClass::Process @ 0x00728E30`, its state machine in
    /// the object turn. An idle Tunnel returns at once (`0x00728E40`). Every
    /// other state reads GetHeight and the Location Z first, then runs its
    /// step, then the tail ([`Self::tunnel_process_tail`]).
    pub(crate) fn process_tunnel(
        &mut self,
        id: u64,
        rules: &RuleSet,
        registry: Option<&OverlayTypeRegistry>,
        effects: FrameEffects<'_>,
    ) -> Result<TunnelProcess, FrameAdvanceError> {
        let mut process = TunnelProcess::default();
        let Some(state) = self
            .substrate
            .entities
            .get(id)
            .and_then(tunnel_runtime)
            .map(TunnelRuntime::state)
        else {
            return Ok(process);
        };
        if state == TunnelState::Idle {
            return Ok(process);
        }
        let frame = self.session.binary_frame();
        let height = self.tunnel_height(id);
        let elapsed = self
            .substrate
            .entities
            .get(id)
            .and_then(tunnel_runtime)
            .is_some_and(|runtime| runtime.delay_elapsed(frame));
        match state {
            TunnelState::Idle => {}
            TunnelState::Turning if height < 0 => self.set_tunnel_state(id, TunnelState::Sinking),
            TunnelState::Turning => self.tunnel_turn_to_dig(id, rules),
            TunnelState::Digging if height < 0 => self.set_tunnel_state(id, TunnelState::Sinking),
            TunnelState::Digging => {
                if elapsed {
                    // 0x00728ED4..0x00728F7B.
                    if let Some(entity) = self.substrate.entities.get_mut(id) {
                        entity
                            .foot_speed
                            .set_speed_fraction_native_bits(NativeF64Bits::ONE.bits());
                    }
                    self.set_tunnel_state(id, TunnelState::Sinking);
                    self.tunnel_dig_effects(id, rules, true);
                    self.detach_all(
                        id,
                        false,
                        crate::sim::world::UninitContext::new(rules, registry)
                            .with_effects(effects),
                    );
                }
            }
            TunnelState::Sinking => self.tunnel_sink(id, rules, registry, effects),
            TunnelState::Underground => self.tunnel_travel(id, rules, registry, effects),
            TunnelState::Rising => self.tunnel_rise(id, rules, registry, effects),
            TunnelState::Surfaced => {
                if elapsed {
                    // 0x0072907F..0x007290F1: Per_Cell_Process(2), then, for a
                    // live object neither in limbo nor falling, Stop_Moving,
                    // idle and FootClass::Stop_Moving (0x004DF0D0).
                    process.bridge_state_changed |= self.per_cell_process(
                        id,
                        super::PerCellReason::Arrival,
                        rules,
                        registry,
                        effects,
                    )?;
                    process.per_cell_ran = true;
                    let settles = self.substrate.entities.get(id).is_some_and(|entity| {
                        entity.is_object_alive()
                            && !entity.lifecycle.in_limbo()
                            && !entity.is_falling_down()
                    });
                    if settles {
                        self.tunnel_stop_moving(id, rules, registry, effects);
                        self.tunnel_settle(id);
                    }
                }
            }
            TunnelState::Abandoned => {
                if elapsed {
                    self.tunnel_settle(id);
                }
            }
        }
        self.tunnel_process_tail(id, rules);
        Ok(process)
    }

    /// Idle again and `FootClass::Stop_Moving @ 0x004DF0D0`'s NavCom clear
    /// (`0x007290E7..0x007290F1`).
    fn tunnel_settle(&mut self, id: u64) {
        self.set_tunnel_state(id, TunnelState::Idle);
        if let Some(entity) = self.substrate.entities.get_mut(id) {
            entity.navigation.stop_moving();
        }
    }

    /// State 1 on the surface (`0x007291F0`): once the body stops turning, a
    /// body not facing the destination turns toward it (Do_Turn
    /// `0x0072A0E0`, `FacingClass::Set_Desired`); a facing one starts the
    /// dig-in delay, digs (sound and animation) and enters state 2.
    fn tunnel_turn_to_dig(&mut self, id: u64, rules: &RuleSet) {
        let frame = self.session.binary_frame();
        let Some(entity) = self.substrate.entities.get(id) else {
            return;
        };
        if entity.body_facing.is_rotating(frame) {
            return;
        }
        let (Some(location), Some(destination)) = (
            self.tunnel_location(id),
            tunnel_runtime(entity).map(TunnelRuntime::destination),
        ) else {
            return;
        };
        // 0x0072920C..0x00729265: atan2(loc.y - dest.y, dest.x - loc.x).
        let facing = crate::util::direction_tables::facing16_from_delta(
            destination.x.wrapping_sub(location.x),
            destination.y.wrapping_sub(location.y),
        );
        if facing != entity.body_facing.current(frame) {
            if let Some(entity) = self.substrate.entities.get_mut(id) {
                super::locomotor_turn::locomotor_do_turn(entity, facing, frame);
            }
            return;
        }
        let rot = self
            .object_type(entity.type_ref(), rules)
            .map_or(0, |object| object.turret_rot);
        let delay = dig_delay(rot, Some(rules.general.tunnel_speed));
        if let Some(runtime) = self.tunnel_runtime_mut(id) {
            runtime.state = TunnelState::Digging;
            runtime.start_delay(frame, delay);
        }
        self.tunnel_dig_effects(id, rules, true);
    }

    /// State 3 (`0x00728F86..0x0072901C`): at or below [`TUNNEL_DEPTH`] the
    /// unit takes Mark(UP) and is burrowed; otherwise it sinks one dig step.
    fn tunnel_sink(
        &mut self,
        id: u64,
        rules: &RuleSet,
        registry: Option<&OverlayTypeRegistry>,
        effects: FrameEffects<'_>,
    ) {
        let Some(location) = self.tunnel_location(id) else {
            return;
        };
        if location.z <= TUNNEL_DEPTH {
            self.foot_mark_remove(id, rules, registry, effects);
            self.set_tunnel_state(id, TunnelState::Underground);
            return;
        }
        let step = dig_step(
            self.tunnel_current_speed(id, rules),
            rules.general.tunnel_speed,
        );
        let z = location.z.wrapping_sub(step).max(TUNNEL_DEPTH);
        self.foot_set_location_marked(id, DriveCoord { z, ..location }, rules, registry, effects);
    }

    /// State 4 (`0x00729580`). Twenty or more leptons from the destination,
    /// the unit steps 19 leptons along the quantized facing toward it, as
    /// Walk's step does (`0x0072980F..0x007298D3`). Nearer, it arrives at
    /// the destination's XY at [`TUNNEL_DEPTH`]: when its Unit
    /// `Can_Enter_Cell` (vt+0x1AC, `0x0073F0A0`, with (-1, -1, NULL, 1))
    /// refuses the cell under it inside the playfield (`0x005785F0`), it
    /// heads under to the nearest burrowable cell, or dies by `C4Warhead=`
    /// when there is none; otherwise it rises (state 5, Mark(DOWN)), with the
    /// dig sound when within 50 leptons of the ground.
    fn tunnel_travel(
        &mut self,
        id: u64,
        rules: &RuleSet,
        registry: Option<&OverlayTypeRegistry>,
        effects: FrameEffects<'_>,
    ) {
        let (Some(location), Some(destination)) = (
            self.tunnel_location(id),
            self.substrate
                .entities
                .get(id)
                .and_then(tunnel_runtime)
                .map(TunnelRuntime::destination),
        ) else {
            return;
        };
        let dx = destination.x.wrapping_sub(location.x);
        let dy = destination.y.wrapping_sub(location.y);
        if crate::util::native_x87::sqrt_approx_length([dx, dy]) >= ARRIVAL_DISTANCE {
            let facing = crate::util::direction_tables::facing16_from_delta(dx, dy);
            let [x, y] = crate::util::native_trig::facing_step_world_xy(
                [location.x, location.y],
                facing,
                UNDERGROUND_STEP,
            );
            self.foot_set_location_marked(
                id,
                DriveCoord { x, y, ..location },
                rules,
                registry,
                effects,
            );
            return;
        }
        let arrival = DriveCoord {
            z: TUNNEL_DEPTH,
            ..destination
        };
        if self.tunnel_arrival_blocked(id, location, rules, registry) {
            match self.tunnel_surface_cell(id, MovementZone::Subterranean) {
                Some((x, y)) => {
                    let next = DriveCoord::cell(x as u16, y as u16, 0);
                    self.tunnel_move_to(id, next, rules, registry, effects);
                    self.foot_set_location_marked(id, arrival, rules, registry, effects);
                }
                None => self.receive_own_health_c4(id, rules, registry, effects),
            }
            return;
        }
        if self.tunnel_height(id) > SURFACE_HEIGHT {
            self.tunnel_dig_effects(id, rules, false);
        }
        self.set_tunnel_state(id, TunnelState::Rising);
        self.foot_mark_put(id, rules, registry, effects);
        self.foot_set_location_marked(id, arrival, rules, registry, effects);
    }

    /// The arrival's test (`0x007295EE..0x00729662`).
    fn tunnel_arrival_blocked(
        &self,
        id: u64,
        location: DriveCoord,
        rules: &RuleSet,
        registry: Option<&OverlayTypeRegistry>,
    ) -> bool {
        let Some(terrain) = self.resolved_terrain.as_ref() else {
            return false;
        };
        let cell = NativeCellQuery::canonical(terrain).lookup_world(location.x, location.y);
        let refused = self
            .foot_can_enter(
                id,
                cell,
                super::infantry_entry::InfantryEntryArgs {
                    direction: -1,
                    height: -1,
                    previous_cell: None,
                },
                rules,
                registry,
            )
            .is_ok_and(|code| code != 0);
        refused
            && crate::sim::cell_rect::cell_is_in_playfield_leptons(
                (location.x, location.y, location.z),
                self.playfield_bounds(),
                Some(terrain),
            )
    }

    /// State 5 (`0x007298F0`): below the floor, the unit rises one dig step
    /// (capped at the floor), digging out (sound and animation) as it
    /// crosses 50 leptons under the ground; at the floor it sees (vt+0x120,
    /// `0x0070ADC0`), starts the after-surfacing delay, stops its speed and
    /// enters state 6.
    fn tunnel_rise(
        &mut self,
        id: u64,
        rules: &RuleSet,
        registry: Option<&OverlayTypeRegistry>,
        effects: FrameEffects<'_>,
    ) {
        let Some(location) = self.tunnel_location(id) else {
            return;
        };
        let floor = self.tunnel_floor(location);
        if location.z < floor {
            let before = self.tunnel_height(id);
            let step = dig_step(
                self.tunnel_current_speed(id, rules),
                rules.general.tunnel_speed,
            );
            let z = location.z.wrapping_add(step).min(floor);
            self.foot_set_location_marked(
                id,
                DriveCoord { z, ..location },
                rules,
                registry,
                effects,
            );
            if before <= SURFACE_HEIGHT && self.tunnel_height(id) > SURFACE_HEIGHT {
                self.tunnel_dig_effects(id, rules, true);
            }
            return;
        }
        self.reveal_uncached_object_sight(
            id,
            false,
            false,
            None,
            crate::sim::world::UninitContext::new(rules, registry).with_effects(effects),
        );
        let frame = self.session.binary_frame();
        let rot = self
            .substrate
            .entities
            .get(id)
            .and_then(|entity| self.object_type(entity.type_ref(), rules))
            .map_or(0, |object| object.turret_rot);
        let delay = dig_delay(rot, None);
        if let Some(runtime) = self.tunnel_runtime_mut(id) {
            runtime.start_delay(frame, delay);
        }
        if let Some(entity) = self.substrate.entities.get_mut(id) {
            entity
                .foot_speed
                .set_speed_fraction_native_bits(NativeF64Bits::POSITIVE_ZERO.bits());
        }
        self.set_tunnel_state(id, TunnelState::Surfaced);
    }

    /// Process's tail (`0x007290F6..0x00729192`): a selected object that is
    /// not the player's is deselected over shroud (`MapClass::IsShrouded @
    /// 0x00586360`; the fog arm's `0x005865E0` is a constant false), then
    /// `TechnoClass @ 0x0070D7A0` retires the damage smoke above
    /// `ConditionYellow` or more than ten leptons under the ground.
    fn tunnel_process_tail(&mut self, id: u64, rules: &RuleSet) {
        let Some(entity) = self.substrate.entities.get(id) else {
            return;
        };
        let deselect = entity.selection.is_selected()
            && !self.house_is_human_player(entity.owner())
            && self.tunnel_location_shrouded(id);
        if deselect {
            self.object_deselect(id);
        }
        let Some(entity) = self.substrate.entities.get(id) else {
            return;
        };
        let above_yellow = self
            .object_type(entity.type_ref(), rules)
            .is_some_and(|object| {
                entity
                    .health
                    .compare_ratio(object.strength, rules.general.condition_yellow)
                    == crate::util::native_x87::MaskedX87Ordering::Greater
            });
        if above_yellow || self.tunnel_height(id) < -10 {
            self.substrate.retire_attached_damage_smoke(id);
        }
    }

    fn tunnel_location_shrouded(&self, id: u64) -> bool {
        let (Some(terrain), Some(house), Some(location)) = (
            self.resolved_terrain.as_ref(),
            self.session.current_house(),
            self.tunnel_location(id),
        ) else {
            return false;
        };
        let cells = NativeCellQuery::isolated(terrain);
        crate::sim::vision::coordinate_is_shrouded(&cells, location, &|cell| {
            Ok(match cell {
                NativeCellIdentity::Real(index) => {
                    let cell = &terrain.cells()[index];
                    self.fog.is_cell_revealed(house, cell.rx, cell.ry)
                }
                NativeCellIdentity::Dummy => false,
            })
        })
        .unwrap_or(false)
    }

    /// The Unit setter's Tunnel arm (`0x007427C0..0x00742C06`), for a
    /// non-null destination at `destination` (its GetCoords, vt+0x48). It
    /// runs only while the active locomotor is a Tunnel that is not moving
    /// and the unit stands on the ground (GetHeight 0). The Tunnel stays in
    /// charge when the unit's navigation should not be on a bridge (vt+0xBC,
    /// `0x004DDC40(0)`), the destination is not a bridge cell (`+0x140 &
    /// 0x100`), neither the unit's navigation cell (vt+0x4C) nor the
    /// destination's refuses burrowing, and the trip is worth a tunnel
    /// ([`Self::unit_tunnel_worthwhile`]). Otherwise a Drive piggybacks over
    /// it (the same install as the Teleporter arm,
    /// [`locomotor_owner::begin_drive_piggyback`]) and is forced onto the
    /// slope under the unit (Drive `Force_New_Slope @ 0x004AFB40`,
    /// `0x00742BE3`).
    ///
    /// [`locomotor_owner::begin_drive_piggyback`]: super::locomotor_owner::begin_drive_piggyback
    pub(crate) fn unit_tunnel_arm(&mut self, id: u64, destination: DriveCoord, rules: &RuleSet) {
        let Some(entity) = self.substrate.entities.get(id) else {
            return;
        };
        let idle_tunnel = entity
            .locomotor
            .as_ref()
            .and_then(|locomotor| locomotor.tunnel_runtime())
            .is_some_and(|runtime| !runtime.is_moving());
        if !idle_tunnel || self.tunnel_height(id) != 0 {
            return;
        }
        let Some(location) = self.tunnel_location(id) else {
            return;
        };
        let destination_bridge = self.resolved_terrain.as_ref().is_some_and(|terrain| {
            let cells = NativeCellQuery::canonical(terrain);
            cells.flags(cells.lookup_world(destination.x, destination.y)) & 0x100 != 0
        });
        let navigation = self.foot_navigation_coordinate(id).unwrap_or(location);
        let on_bridge = self.resolved_terrain.as_ref().is_some_and(|terrain| {
            super::ground_pose::navigation_should_be_on_bridge(
                &NativeCellQuery::canonical(terrain),
                navigation,
                location,
                entity.on_bridge(),
                entity.low_bridge_tube_state.is_some(),
            )
            .unwrap_or(false)
        });
        let tunnels = !on_bridge
            && !destination_bridge
            && self.tunnel_burrowable_at(navigation)
            && self.tunnel_burrowable_at(destination)
            && self.unit_tunnel_worthwhile(
                id,
                super::foot_path::coord_cell(navigation),
                super::foot_path::coord_cell(destination),
                rules,
            );
        if tunnels {
            return;
        }
        let frame = self.session.binary_frame();
        let slope = self.resolved_terrain.as_ref().map_or(0, |terrain| {
            let cells = NativeCellQuery::canonical(terrain);
            cells
                .ground_fields(cells.lookup_world(location.x, location.y))
                .1
        });
        if let Some(entity) = self.substrate.entities.get_mut(id)
            && super::locomotor_owner::begin_drive_piggyback(entity, frame)
        {
            super::slope_transition::snap_after_successful_unlimbo(entity, slope, frame);
        }
    }

    /// `UnitClass @ 0x00746000`: whether a trip from `from` to `to` should
    /// tunnel. A unit under the ground, a destination its Normal zone cannot
    /// reach (`MapClass @ 0x0056D100`), one twelve or more cells away, an
    /// endpoint outside the playfield (`0x00578460`) or a ground route
    /// longer than fifteen (`PathfinderClass::EstimateZoneCost @ 0x0042D170`)
    /// tunnels; the same cell or a neighbour drives.
    fn unit_tunnel_worthwhile(
        &self,
        id: u64,
        from: (i16, i16),
        to: (i16, i16),
        rules: &RuleSet,
    ) -> bool {
        if self.tunnel_height(id) < 0 {
            return true;
        }
        if !self.tunnel_zone_reachable(from, to) {
            return true;
        }
        if from == to {
            return false;
        }
        let distance = (i32::from(to.0) - i32::from(from.0))
            .abs()
            .max((i32::from(to.1) - i32::from(from.1)).abs());
        if distance == 1 {
            return false;
        }
        if distance >= 12 {
            return true;
        }
        let terrain = self.resolved_terrain.as_ref();
        let bounds = self.playfield_bounds();
        let inside = |cell: (i16, i16)| {
            crate::sim::cell_rect::cell_is_in_playfield_height_aware(
                (i32::from(cell.0), i32::from(cell.1)),
                bounds,
                terrain,
            )
        };
        if !inside(from) || !inside(to) {
            return true;
        }
        let to_bridge = terrain.is_some_and(|terrain| {
            terrain.native_cell_flags(terrain.native_cell_identity(to)) & 0x100 != 0
        });
        // 0x007460B6..0x007460E9: the destination's bridge flag is the
        // fifth argument, after the Foot and a zero.
        self.estimate_zone_cost(id, from, to, false, to_bridge, rules)
            .map_or(true, |cost| cost > 15)
    }

    /// `MapClass::Can_Reach_Zone @ 0x0056D100` as `0x00746030` asks it:
    /// MovementZone Normal, no bridge on either end, no fringe. A world
    /// without zone topology answers reachable.
    fn tunnel_zone_reachable(&self, from: (i16, i16), to: (i16, i16)) -> bool {
        use crate::sim::pathfinding::zone_map::ZoneQueryCell;
        let (Some(terrain), Some(zones), Some(bounds), Some(height)) = (
            self.resolved_terrain.as_ref(),
            self.zone_grid.as_ref(),
            self.playfield_bounds(),
            self.playfield_size_height(),
        ) else {
            return true;
        };
        zones
            .can_reach_native(
                &NativeCellQuery::canonical(terrain),
                ZoneQueryCell::Copied(from),
                ZoneQueryCell::Copied(to),
                MovementZone::Normal,
                false,
                false,
                false,
                bounds,
                (bounds.base, height),
            )
            .unwrap_or(true)
    }
}

#[cfg(test)]
#[path = "tunnel_movement_tests.rs"]
mod tests;
