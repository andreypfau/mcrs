use glam::DVec3;
use mcrs_voxel::BlockPos;

use crate::Aabb;
use crate::BlockAction;
use crate::BlockFlags;
use crate::Body;
use crate::BodyView;
use crate::CollisionWorld;
use crate::InsideEffect;
use crate::Living;
use crate::MovePath;
use crate::Mover;
use crate::ShapeClass;
use crate::StuckIgnore;
use crate::View;
use crate::block_scan::floor_block;
use crate::block_scan::state_at;
use crate::move_body::offset_block;
use crate::support::EFFECT_OFFSET;
use crate::tolerance::CLIP;
use crate::tolerance::COLLISION;

/// The values of one entity for the block effects of its tick.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct EffectContext<'a> {
    pub(crate) mover: &'a Mover,
    pub(crate) stuck_ignore: StuckIgnore,
    pub(crate) powder_snow_needs_inside: bool,
    /// The path through the blocks.
    pub(crate) path: MovePath,
    /// The vertical displacement of the moves of the tick.
    pub(crate) dy: f64,
    /// A player with the flight state on ignores every stuck multiplier.
    pub(crate) flying: bool,
    /// False for the server copy of a player, whose fall distance comes from the moves of the
    /// client.
    pub(crate) updates_fall: bool,
}

/// Applies the step action, the blocks inside the path of the box, and the fall distance.
pub(crate) fn apply<W: CollisionWorld, B: BodyView>(
    view: View<'_, W, B>,
    context: &EffectContext<'_>,
    body: &mut Body,
    living: Option<&mut Living>,
    actions: &mut Vec<BlockAction>,
) {
    let table = &view.tables.collision;
    let mover = context.mover;
    if body.flags.on_ground {
        let at = offset_block(view.world, table, body, EFFECT_OFFSET);
        let block = table.state(state_at(view.world, at));
        if block.flags.contains(BlockFlags::STEP_ACTION) {
            actions.push(BlockAction::Step(at));
        }
        let vertical = body.velocity.y.abs();
        if block.flags.contains(BlockFlags::SLIME) && vertical < 0.1 && !mover.shift {
            let factor = 0.4 + vertical * 0.2;
            body.velocity.x *= factor;
            body.velocity.z *= factor;
        }
    }
    let touched_powder_snow = inside_blocks(view, context, body, living.as_deref(), actions);
    if let Some(living) = living {
        living.in_powder_snow = touched_powder_snow;
    }
    if context.updates_fall {
        fall_distance(view, context, body, actions);
    }
}

/// Applies the blocks that the shrunk box passed through; returns true when it touched powder
/// snow.
fn inside_blocks<W: CollisionWorld, B: BodyView>(
    view: View<'_, W, B>,
    context: &EffectContext<'_>,
    body: &mut Body,
    living: Option<&Living>,
    actions: &mut Vec<BlockAction>,
) -> bool {
    let table = &view.tables.collision;
    let mover = context.mover;
    let from = Aabb::of_body(context.path.start, mover.width, mover.height).deflate(CLIP);
    let to = Aabb::of_body(context.path.end, mover.width, mover.height).deflate(CLIP);
    let delta = to.min - from.min;
    let low = from.min.min(to.min);
    let high = from.max.max(to.max);
    let mut touched_powder_snow = false;
    for z in floor_block(low.z)..=floor_block(high.z) {
        for y in floor_block(low.y)..=floor_block(high.y) {
            for x in floor_block(low.x)..=floor_block(high.x) {
                let at = BlockPos::new(x, y, z);
                let block = table.state(state_at(view.world, at));
                let relevant = block.inside != InsideEffect::None
                    || block
                        .flags
                        .intersects(BlockFlags::INSIDE_ACTION | BlockFlags::FALL_DAMAGE_RESETTING);
                if !relevant {
                    continue;
                }
                let cell = Aabb::new(at.as_dvec3(), at.as_dvec3() + DVec3::ONE);
                let Some(part) = sweep_part(&from, delta, &cell) else {
                    continue;
                };
                if context.updates_fall
                    && block.flags.contains(BlockFlags::FALL_DAMAGE_RESETTING)
                    && centre_resets_fall(context, delta, part, &cell)
                {
                    body.fall_distance = 0.0;
                }
                if block.flags.contains(BlockFlags::INSIDE_ACTION) {
                    actions.push(BlockAction::Inside(at));
                }
                if block.inside == InsideEffect::PowderSnow {
                    touched_powder_snow = true;
                }
                inside_effect(view, context, body, living, block.inside, at);
            }
        }
    }
    touched_powder_snow
}

/// True when the centre of the box touches the cell in the part of the path in which the box
/// touches it. A part shorter than the square root of `1.0E-7` never resets.
fn centre_resets_fall(context: &EffectContext<'_>, delta: DVec3, part: Part, cell: &Aabb) -> bool {
    let length = part.end - part.start;
    if delta.length_squared() * length * length < COLLISION {
        return false;
    }
    let lift = DVec3::new(0.0, f64::from(context.mover.height) / 2.0, 0.0);
    let from = context.path.start + lift + delta * part.start;
    let centre = Aabb::new(from, from);
    sweep_part(&centre, delta * length, cell).is_some()
}

fn inside_effect<W: CollisionWorld, B: BodyView>(
    view: View<'_, W, B>,
    context: &EffectContext<'_>,
    body: &mut Body,
    living: Option<&Living>,
    effect: InsideEffect,
    at: BlockPos,
) {
    let multiplier = match effect {
        InsideEffect::None => return,
        InsideEffect::Honey => {
            slide(context.mover, body, at);
            return;
        }
        InsideEffect::Cobweb if living.is_some_and(|living| living.effects.weaving) => {
            DVec3::new(0.5, 0.25, 0.5)
        }
        InsideEffect::Cobweb => DVec3::new(0.25, f64::from(0.05_f32), 0.25),
        InsideEffect::SweetBerryBush => DVec3::new(f64::from(0.8_f32), 0.75, f64::from(0.8_f32)),
        InsideEffect::PowderSnow => {
            if context.powder_snow_needs_inside {
                let floor = BlockPos::new(
                    floor_block(body.position.x),
                    floor_block(body.position.y),
                    floor_block(body.position.z),
                );
                let class = view
                    .tables
                    .collision
                    .state(state_at(view.world, floor))
                    .class;
                if class != ShapeClass::PowderSnow {
                    return;
                }
            }
            DVec3::new(f64::from(0.9_f32), 1.5, f64::from(0.9_f32))
        }
    };
    if context.flying || context.stuck_ignore.ignores(effect) {
        return;
    }
    body.stuck = multiplier;
    body.fall_distance = 0.0;
}

/// The slide on the side of a honey block.
fn slide(mover: &Mover, body: &mut Body, at: BlockPos) {
    let position = body.position;
    let velocity = body.velocity;
    let drag = f64::from(0.98_f32);
    let falling = velocity.y / drag + 0.08;
    let half_width = f64::from(mover.width / 2.0);
    let reach = 0.4375 + half_width;
    let beside = (f64::from(at.x) + 0.5 - position.x).abs() + COLLISION > reach
        || (f64::from(at.z) + 0.5 - position.z).abs() + COLLISION > reach;
    if body.flags.on_ground
        || position.y > f64::from(at.y) + 0.9375 - COLLISION
        || falling >= -0.08
        || !beside
    {
        return;
    }
    if falling < -0.13 {
        let factor = -0.05 / falling;
        body.velocity.x *= factor;
        body.velocity.z *= factor;
    }
    body.velocity.y = (-0.05 - 0.08) * drag;
    body.fall_distance = 0.0;
}

fn fall_distance<W: CollisionWorld, B: BodyView>(
    view: View<'_, W, B>,
    context: &EffectContext<'_>,
    body: &mut Body,
    actions: &mut Vec<BlockAction>,
) {
    if context.dy < 0.0 {
        body.fall_distance -= context.dy;
    }
    if body.flags.on_ground {
        if body.fall_distance > 0.0 {
            let at = offset_block(view.world, &view.tables.collision, body, EFFECT_OFFSET);
            actions.push(BlockAction::Fall {
                at,
                distance: body.fall_distance,
            });
        }
        body.fall_distance = 0.0;
    }
}

/// The fractions of the path between which the box overlaps a cell.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Part {
    start: f64,
    end: f64,
}

/// The part of the path in which the box, moved by `delta`, overlaps the cell with a volume that
/// is more than zero.
fn sweep_part(from: &Aabb, delta: DVec3, cell: &Aabb) -> Option<Part> {
    let mut low = f64::NEG_INFINITY;
    let mut high = f64::INFINITY;
    for ((start_min, start_max), (cell_min, cell_max), step) in [
        ((from.min.x, from.max.x), (cell.min.x, cell.max.x), delta.x),
        ((from.min.y, from.max.y), (cell.min.y, cell.max.y), delta.y),
        ((from.min.z, from.max.z), (cell.min.z, cell.max.z), delta.z),
    ] {
        if step == 0.0 {
            if start_min >= cell_max || start_max <= cell_min {
                return None;
            }
            continue;
        }
        let enter = (cell_min - start_max) / step;
        let leave = (cell_max - start_min) / step;
        low = low.max(enter.min(leave));
        high = high.min(enter.max(leave));
    }
    (low < high && low < 1.0 && high > 0.0).then(|| Part {
        start: low.max(0.0),
        end: high.min(1.0),
    })
}
