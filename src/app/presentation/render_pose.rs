//! One object's draw transform at the current binary frame, read fresh for
//! each draw and never retained across draws.

use crate::sim::game_entity::GameEntity;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct RenderPose {
    pub screen: [f32; 2],
    pub body: u16,
    pub turret: u16,
    pub barrel: u16,
    /// Bank direction read from the native SecondaryFacing owner at this
    /// binary frame, without advancing or duplicating its timer.
    pub fly_bank_direction: Option<crate::sim::movement::facing_class::RotationDirection>,
    pub angles: [f32; 2],
    /// The Rocket or Tunnel body pitch ([`body_pitch`]).
    pub rocket_pitch: f32,
    pub visual_lift: f32,
    pub recoil: ([f32; 2], bool),
    pub slope: Option<crate::sim::movement::slope_transition::SlopeRenderPhase>,
}

/// Fly DrawMatrix4CF632/4CF659 reads SecondaryFacing for both its cache key
/// and rotation, like its separate ShadowMatrix. PrimaryFacing may already
/// point elsewhere while the plane turns. Other locomotors keep their owner.
pub(crate) fn body_draw_facing(entity: &GameEntity, binary_frame: u32) -> u16 {
    draw_facing(
        entity.locomotor.as_ref(),
        entity.barrel_facing.as_ref(),
        &entity.body_facing,
        binary_frame,
    )
}

/// [`body_draw_facing`] of an object's presentation record.
pub(crate) fn record_body_draw_facing(
    entity: &crate::app::presentation::frame::ObjectRecord,
    binary_frame: u32,
) -> u16 {
    draw_facing(
        entity.locomotor.as_ref(),
        entity.barrel_facing.as_ref(),
        &entity.body_facing,
        binary_frame,
    )
}

fn draw_facing(
    locomotor: Option<&crate::sim::movement::locomotor::LocomotorState>,
    barrel: Option<&crate::sim::movement::FacingClass>,
    body: &crate::sim::movement::FacingClass,
    binary_frame: u32,
) -> u16 {
    if locomotor
        .is_some_and(|loco| loco.active_kind() == crate::rules::locomotor_type::LocomotorKind::Fly)
    {
        barrel.map_or_else(
            || body.current(binary_frame),
            |facing| facing.current(binary_frame),
        )
    } else {
        body.current(binary_frame)
    }
}

/// The body pitch the Rocket and Tunnel Draw_Matrix share: both rotate the
/// facing matrix (`0x0055A730`'s RotateZ) about Y (`0x005AF080`). Rocket
/// (`0x00663470`) negates its CurrentPitch; Tunnel (`0x00729B40`) rotates by
/// its dig pitch as it is, so it is negated into the same convention.
pub(crate) fn body_pitch(
    locomotor: Option<&crate::sim::movement::locomotor::LocomotorState>,
    binary_frame: u32,
) -> f32 {
    let Some(locomotor) = locomotor else {
        return 0.;
    };
    if let Some(rocket) = locomotor.rocket_runtime() {
        return rocket.draw_pitch();
    }
    locomotor
        .tunnel_runtime()
        .map_or(0., |tunnel| -tunnel.draw_pitch(binary_frame))
}

fn fly_bank_direction(
    entity: &GameEntity,
    binary_frame: u32,
) -> Option<crate::sim::movement::facing_class::RotationDirection> {
    entity
        .locomotor
        .as_ref()
        .filter(|loco| loco.active_kind() == crate::rules::locomotor_type::LocomotorKind::Fly)
        .and_then(|_| entity.barrel_facing.as_ref())
        .and_then(|facing| facing.rotation_direction_at(binary_frame))
}

impl RenderPose {
    pub(crate) fn read(entity: &GameEntity, frame: u32) -> Self {
        let (x, y) = crate::render::locomotor_visual::screen_position(entity);
        let body = body_draw_facing(entity, frame);
        Self {
            screen: [x, y],
            body,
            barrel: entity.barrel_elevation().current(frame),
            fly_bank_direction: fly_bank_direction(entity, frame),
            angles: entity
                .rocking
                .as_ref()
                .map_or([0.; 2], |r| r.angles_native()),
            rocket_pitch: body_pitch(entity.locomotor.as_ref(), frame),
            visual_lift: f32::from(entity.ion_blast_lift_pixels()),
            recoil: entity.voxel_recoil(),
            slope: crate::sim::movement::slope_transition::state_for_entity(entity)
                .map(|s| s.render_phase(frame.wrapping_sub(1))),
            turret: entity
                .barrel_facing
                .as_ref()
                .map_or(body, |f| f.current(frame)),
        }
    }
}
