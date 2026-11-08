use glam::DVec3;
use mcrs_voxel::BlockPos;

use crate::Aabb;
use crate::BlockAction;
use crate::BlockFlags;
use crate::Body;
use crate::BodyView;
use crate::CollisionWorld;
use crate::InsideEffect;
use crate::InsideShape;
use crate::Living;
use crate::MovePath;
use crate::Mover;
use crate::QueryVariant;
use crate::ShapeClass;
use crate::StateCollision;
use crate::StuckIgnore;
use crate::View;
use crate::block_scan::ShapeContext;
use crate::block_scan::any_section;
use crate::block_scan::block_at;
use crate::block_scan::entity_shape;
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
    let [x0, y0, z0] = low.to_array().map(floor_block);
    let [x1, y1, z1] = high.to_array().map(floor_block);
    if !any_section(view.world, [x0, y0, z0], [x1, y1, z1], |masks| masks.inside) {
        return false;
    }
    for z in z0..=z1 {
        for y in y0..=y1 {
            for x in x0..=x1 {
                let at = BlockPos::new(x, y, z);
                let block = table.state(state_at(view.world, at));
                if !block.acts_inside() {
                    continue;
                }
                let cell = Aabb::new(at.as_dvec3(), at.as_dvec3() + DVec3::ONE);
                let Some(part) = sweep_part(&from, delta, &cell) else {
                    continue;
                };
                let shape = inside_boxes(view, context, body, block, at);
                if context.updates_fall
                    && block.flags.contains(BlockFlags::FALL_DAMAGE_RESETTING)
                    && feet_reset_fall(context, delta, part, shape, at)
                {
                    body.fall_distance = 0.0;
                }
                if let Some(boxes) = shape
                    && !centre_touches(&from, delta, part, boxes, at)
                {
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

/// The boxes of the inside shape of a block, relative to its position; `None` for the full cube.
fn inside_boxes<'t, W: CollisionWorld, B: BodyView>(
    view: View<'t, W, B>,
    context: &EffectContext<'_>,
    body: &Body,
    block: StateCollision,
    at: BlockPos,
) -> Option<&'t [Aabb]> {
    let table = &view.tables.collision;
    match block.inside_shape {
        InsideShape::FullCube => None,
        InsideShape::Boxes(range) => Some(table.inside_boxes(range)),
        InsideShape::Collision => {
            let entity = context.mover.query_entity(body);
            let shape_context = ShapeContext {
                entity: &entity,
                feet: body.position.y,
                variant: QueryVariant::Collision,
            };
            let deep = entity_shape(table, block, at, &shape_context)
                .is_some_and(|shape| shape == table.powder_snow_deep);
            deep.then(|| table.inside_boxes(table.powder_snow_deep_inside))
        }
    }
}

/// True when the bottom centre of the box touches the inside shape in the part of the path in
/// which the box touches the cell. A part shorter than the square root of `1.0E-7` never resets.
fn feet_reset_fall(
    context: &EffectContext<'_>,
    delta: DVec3,
    part: Part,
    shape: Option<&[Aabb]>,
    at: BlockPos,
) -> bool {
    let length = part.end - part.start;
    if delta.length_squared() * length * length < COLLISION {
        return false;
    }
    let from = context.path.start + delta * part.start;
    let feet = Aabb::new(from, from);
    let cell = [Aabb::new(DVec3::ZERO, DVec3::ONE)];
    shape
        .unwrap_or(&cell)
        .iter()
        .any(|bounds| sweep_part(&feet, delta * length, &bounds.translate(at.as_dvec3())).is_some())
}

/// True when the centre of the box, along the part of the path, touches a box of the inside
/// shape grown by half the size of the box less `1.0E-7`.
fn centre_touches(from: &Aabb, delta: DVec3, part: Part, boxes: &[Aabb], at: BlockPos) -> bool {
    let half = from.size() / 2.0 - DVec3::splat(COLLISION);
    let start = (from.min + from.max) / 2.0 + delta * part.start;
    let centre = Aabb::new(start, start);
    let along = delta * (part.end - part.start);
    boxes.iter().any(|bounds| {
        let grown = bounds.translate(at.as_dvec3()).inflate(half);
        sweep_part(&centre, along, &grown).is_some()
    })
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
                let floor = block_at(body.position);
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
