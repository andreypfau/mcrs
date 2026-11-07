use glam::DVec3;
use mcrs_voxel::BlockPos;

use crate::BlockFlags;
use crate::Body;
use crate::BodyView;
use crate::CollisionWorld;
use crate::MoveFlags;
use crate::PlayerState;
use crate::View;
use crate::block_scan::floor_block;
use crate::block_scan::state_at;
use crate::move_body::offset_block;
use crate::support::EFFECT_OFFSET;
use crate::support::MOVEMENT_OFFSET;
use crate::travel;
use crate::travel::modifier;

/// What the server observed of one tick of a player.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Observation {
    /// The displacement of the tick.
    pub displacement: DVec3,
    /// The flags at the end of the tick, from the move packet and the re-application.
    pub end_flags: MoveFlags,
    /// The supporting block at the end of the tick.
    pub end_support: Option<BlockPos>,
    /// The flight state after step 6 of the tick.
    pub flight: bool,
    pub jump: bool,
    pub shift: bool,
}

/// The velocities that the observed tick can end with: three when one horizontal flag hides
/// which axes clipped.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VelocityCandidates {
    values: [DVec3; 3],
    count: usize,
    /// False when a ceiling stopped a flying player: then `y` is not known.
    pub vertical_known: bool,
}

impl VelocityCandidates {
    #[must_use]
    pub fn as_slice(&self) -> &[DVec3] {
        self.values.get(..self.count).unwrap_or(&[])
    }
}

/// The velocity at the end of an observed player tick, before the zero cutoff of the next tick.
#[must_use]
pub fn observed_velocity<W: CollisionWorld, B: BodyView>(
    view: View<'_, W, B>,
    start: &PlayerState,
    observation: &Observation,
) -> VelocityCandidates {
    let table = &view.tables.collision;
    let flags = observation.end_flags;
    let clips: &[(bool, bool)] = if flags.horizontal_collision {
        &[(true, false), (false, true), (true, true)]
    } else {
        &[(false, false)]
    };
    let end = Body {
        position: start.position + observation.displacement,
        flags,
        support: observation.end_support,
        ..Body::default()
    };
    let speed = speed_factor(view, &end, start.attributes.movement_efficiency as f32);
    let start_body = start.body();
    let ground = if start.flags.on_ground {
        let below = offset_block(view.world, table, &start_body, MOVEMENT_OFFSET);
        let friction = table.state(state_at(view.world, below)).motion.friction;
        modifier(friction, start.attributes.friction_modifier as f32)
    } else {
        1.0
    };
    let air_drag = start.attributes.air_drag_modifier as f32;
    let horizontal = f64::from(ground * modifier(0.91, air_drag));
    let vertical_drag = f64::from(modifier(0.98, air_drag));
    let stuck = start.stuck.length_squared() > 1.0e-7;
    let delta = if stuck {
        DVec3::ZERO
    } else {
        observation.displacement
    };
    let mut vertical = if flags.vertical_collision {
        0.0
    } else {
        delta.y
    };
    let moved_vertical = vertical;
    if !stuck && climbs(view, start) && (flags.horizontal_collision || observation.jump) {
        vertical = 0.2;
    }
    let vy = if observation.flight {
        0.6 * vertical
    } else {
        let y = match start.effects.levitation {
            Some(level) => vertical + (0.05 * (f64::from(level) + 1.0) - vertical) * 0.2,
            None => {
                let gravity = travel::effective_gravity(
                    start.attributes.gravity,
                    start.effects.slow_falling,
                    moved_vertical,
                );
                vertical - gravity
            }
        };
        y * vertical_drag
    };
    let effect = offset_block(view.world, table, &end, EFFECT_OFFSET);
    let slime = flags.on_ground
        && table
            .state(state_at(view.world, effect))
            .flags
            .contains(BlockFlags::SLIME)
        && vy.abs() < 0.1
        && !observation.shift;
    let mut values = [DVec3::ZERO; 3];
    for (value, &(clip_x, clip_z)) in values.iter_mut().zip(clips) {
        let ux = if clip_x { 0.0 } else { delta.x } * speed;
        let uz = if clip_z { 0.0 } else { delta.z } * speed;
        let mut vx = ux * horizontal;
        let mut vz = uz * horizontal;
        if slime {
            let factor = 0.4 + vy.abs() * 0.2;
            vx *= factor;
            vz *= factor;
        }
        *value = DVec3::new(vx, vy, vz);
    }
    VelocityCandidates {
        values,
        count: clips.len(),
        vertical_known: !(observation.flight && flags.vertical_collision),
    }
}

/// The speed factor of the blocks under the body, mixed with `movement_efficiency`.
fn speed_factor<W: CollisionWorld, B: BodyView>(
    view: View<'_, W, B>,
    body: &Body,
    efficiency: f32,
) -> f64 {
    let table = &view.tables.collision;
    let at = BlockPos::new(
        floor_block(body.position.x),
        floor_block(body.position.y),
        floor_block(body.position.z),
    );
    let own = table.state(state_at(view.world, at));
    let mut factor = own.motion.speed_factor;
    if !own.flags.contains(BlockFlags::OWN_SPEED_FACTOR) && factor == 1.0 {
        let below = offset_block(view.world, table, body, MOVEMENT_OFFSET);
        factor = table.state(state_at(view.world, below)).motion.speed_factor;
    }
    f64::from(factor + efficiency * (1.0 - factor))
}

fn climbs<W: CollisionWorld, B: BodyView>(view: View<'_, W, B>, start: &PlayerState) -> bool {
    let at = BlockPos::new(
        floor_block(start.position.x),
        floor_block(start.position.y),
        floor_block(start.position.z),
    );
    view.tables
        .collision
        .state(state_at(view.world, at))
        .flags
        .contains(BlockFlags::CLIMBABLE)
}
