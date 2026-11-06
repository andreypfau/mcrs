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
    /// The position at the start of the tick: the start of the path through the blocks.
    pub(crate) start: DVec3,
    /// The vertical displacement of the moves of the tick.
    pub(crate) dy: f64,
    /// A player with the flight state on ignores every stuck multiplier.
    pub(crate) flying: bool,
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
    // The vanilla server resets the fall distance for the centre path before it adds the fall of
    // the tick, and resets it for a stuck multiplier after: an entity that descends a ladder
    // keeps the fall of one tick, and one that falls into a cobweb ends the tick at 0.
    if living.is_some() && centre_resets_fall(view, context, body) {
        body.fall_distance = 0.0;
    }
    fall_distance(view, context, body, actions);
    let touched_powder_snow = inside_blocks(view, context, body, living.as_deref(), actions);
    if let Some(living) = living {
        living.in_powder_snow = touched_powder_snow;
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
    let from = Aabb::of_body(context.start, mover.width, mover.height).deflate(CLIP);
    let to = Aabb::of_body(body.position, mover.width, mover.height).deflate(CLIP);
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
                    || block.flags.contains(BlockFlags::INSIDE_ACTION);
                if !relevant {
                    continue;
                }
                let cell = Aabb::new(at.as_dvec3(), at.as_dvec3() + DVec3::ONE);
                if !sweeps_through(&from, delta, &cell) {
                    continue;
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

/// True when the centre of the box passed through a block with the tag `fall_damage_resetting`.
/// The vanilla server keeps the fall distance of an item that falls through a sweet berry bush,
/// so only a living entity reads this rule.
fn centre_resets_fall<W: CollisionWorld, B: BodyView>(
    view: View<'_, W, B>,
    context: &EffectContext<'_>,
    body: &Body,
) -> bool {
    let table = &view.tables.collision;
    let lift = DVec3::new(0.0, f64::from(context.mover.height) / 2.0, 0.0);
    let from = context.start + lift;
    let to = body.position + lift;
    let centre = Aabb::new(from, from);
    let low = from.min(to);
    let high = from.max(to);
    for z in floor_block(low.z)..=floor_block(high.z) {
        for y in floor_block(low.y)..=floor_block(high.y) {
            for x in floor_block(low.x)..=floor_block(high.x) {
                let at = BlockPos::new(x, y, z);
                let block = table.state(state_at(view.world, at));
                let cell = Aabb::new(at.as_dvec3(), at.as_dvec3() + DVec3::ONE);
                if block.flags.contains(BlockFlags::FALL_DAMAGE_RESETTING)
                    && sweeps_through(&centre, to - from, &cell)
                {
                    return true;
                }
            }
        }
    }
    false
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

/// True when the box, moved by `delta` along the path, overlaps the cell with a volume that is
/// more than zero at some point of the path.
fn sweeps_through(from: &Aabb, delta: DVec3, cell: &Aabb) -> bool {
    let mut low = f64::NEG_INFINITY;
    let mut high = f64::INFINITY;
    for ((start_min, start_max), (cell_min, cell_max), step) in [
        ((from.min.x, from.max.x), (cell.min.x, cell.max.x), delta.x),
        ((from.min.y, from.max.y), (cell.min.y, cell.max.y), delta.y),
        ((from.min.z, from.max.z), (cell.min.z, cell.max.z), delta.z),
    ] {
        if step == 0.0 {
            if start_min >= cell_max || start_max <= cell_min {
                return false;
            }
            continue;
        }
        let enter = (cell_min - start_max) / step;
        let leave = (cell_max - start_min) / step;
        low = low.max(enter.min(leave));
        high = high.min(enter.max(leave));
    }
    low < high && low < 1.0 && high > 0.0
}
