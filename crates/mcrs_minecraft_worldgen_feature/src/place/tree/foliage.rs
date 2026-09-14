use crate::block_predicate::Direction;
use crate::placer::WorldGenVolume;
use crate::tree::{FoliagePlacer, UnitFloat};
use bevy_math::IVec3;
use mcrs_minecraft_core::BlockPos;
use mcrs_minecraft_core::value_provider::IntProvider;
use mcrs_minecraft_random::Random;
use mcrs_minecraft_random::xoroshiro::XoroshiroRandom;

use super::trunk::{Axis, FoliageAttachment, TreeContext, dist_manhattan};

#[derive(Clone, Debug)]
pub struct Foliage(pub FoliagePlacer);

impl Foliage {
    /// `radius` and `offset`, which every one of the twelve carries and samples
    /// in that order — `radius` with the tree's other sizes, `offset` once per
    /// attachment.
    pub fn base(&self) -> (&IntProvider, &IntProvider) {
        use FoliagePlacer::*;
        match &self.0 {
            Blob { radius, offset, .. }
            | Bush { radius, offset, .. }
            | Fancy { radius, offset, .. }
            | Spruce { radius, offset, .. }
            | Pine { radius, offset, .. }
            | Acacia { radius, offset }
            | DarkOak { radius, offset }
            | MegaJungle { radius, offset, .. }
            | MegaPine { radius, offset, .. }
            | RandomSpread { radius, offset, .. }
            | Cherry { radius, offset, .. }
            | Poplar { radius, offset, .. } => (radius, offset),
        }
    }

    pub fn foliage_height(&self, rng: &mut XoroshiroRandom, tree_height: i32) -> i32 {
        match &self.0 {
            FoliagePlacer::Blob { height, .. }
            | FoliagePlacer::Bush { height, .. }
            | FoliagePlacer::Fancy { height, .. }
            | FoliagePlacer::MegaJungle { height, .. } => height.0,
            FoliagePlacer::Spruce { trunk_height, .. } => {
                4.max(tree_height - trunk_height.sample(rng))
            }
            FoliagePlacer::Pine { height, .. } => height.sample(rng),
            FoliagePlacer::Acacia { .. } => 0,
            FoliagePlacer::DarkOak { .. } => 4,
            FoliagePlacer::MegaPine { crown_height, .. } => crown_height.sample(rng),
            FoliagePlacer::RandomSpread { foliage_height, .. } => foliage_height.sample(rng),
            FoliagePlacer::Cherry { height, .. } | FoliagePlacer::Poplar { height, .. } => {
                height.sample(rng)
            }
        }
    }

    pub fn foliage_radius(&self, rng: &mut XoroshiroRandom, trunk_height: i32) -> i32 {
        let radius = self.base().0.sample(rng);
        match &self.0 {
            FoliagePlacer::Pine { .. } => radius + rng.next_i32_bound((trunk_height + 1).max(1)),
            _ => radius,
        }
    }

    pub fn create_foliage<W: WorldGenVolume>(
        &self,
        cx: &mut TreeContext<'_, W>,
        rng: &mut XoroshiroRandom,
        attachment: FoliageAttachment,
        foliage_height: i32,
        leaf_radius: i32,
    ) {
        let offset = self.base().1.sample(rng);
        let double_trunk = attachment.double_trunk;
        let radius_offset = attachment.radius_offset_xz;

        match &self.0 {
            FoliagePlacer::Blob { .. } => {
                for yo in (offset - foliage_height..=offset).rev() {
                    let current_radius = 0.max(leaf_radius + radius_offset - 1 - yo / 2);
                    self.place_leaves_row(
                        cx,
                        rng,
                        attachment.pos,
                        current_radius,
                        yo,
                        double_trunk,
                    );
                }
            }
            FoliagePlacer::Bush { .. } => {
                for yo in (offset - foliage_height..=offset).rev() {
                    let current_radius = leaf_radius + radius_offset - 1 - yo;
                    self.place_leaves_row(
                        cx,
                        rng,
                        attachment.pos,
                        current_radius,
                        yo,
                        double_trunk,
                    );
                }
            }
            FoliagePlacer::Fancy { .. } => {
                for yo in (offset - foliage_height..=offset).rev() {
                    let current_radius =
                        leaf_radius + i32::from(yo != offset && yo != offset - foliage_height);
                    self.place_leaves_row(
                        cx,
                        rng,
                        attachment.pos,
                        current_radius,
                        yo,
                        double_trunk,
                    );
                }
            }
            FoliagePlacer::Spruce { .. } => {
                let mut current_radius = rng.next_i32_bound(2);
                let mut max_radius = 1;
                let mut min_radius = 0;
                for yo in (-foliage_height..=offset).rev() {
                    self.place_leaves_row(
                        cx,
                        rng,
                        attachment.pos,
                        current_radius,
                        yo,
                        double_trunk,
                    );
                    if current_radius >= max_radius {
                        current_radius = min_radius;
                        min_radius = 1;
                        max_radius = (max_radius + 1).min(leaf_radius + radius_offset);
                    } else {
                        current_radius += 1;
                    }
                }
            }
            FoliagePlacer::Pine { .. } => {
                let mut current_radius = 0;
                for yo in (offset - foliage_height..=offset).rev() {
                    self.place_leaves_row(
                        cx,
                        rng,
                        attachment.pos,
                        current_radius,
                        yo,
                        double_trunk,
                    );
                    if current_radius >= 1 && yo == offset - foliage_height + 1 {
                        current_radius -= 1;
                    } else if current_radius < leaf_radius + radius_offset {
                        current_radius += 1;
                    }
                }
            }
            FoliagePlacer::Acacia { .. } => {
                let foliage_pos = attachment.pos + IVec3::Y * offset;
                self.place_leaves_row(
                    cx,
                    rng,
                    foliage_pos,
                    leaf_radius + radius_offset,
                    -1 - foliage_height,
                    double_trunk,
                );
                self.place_leaves_row(
                    cx,
                    rng,
                    foliage_pos,
                    leaf_radius - 1,
                    -foliage_height,
                    double_trunk,
                );
                self.place_leaves_row(
                    cx,
                    rng,
                    foliage_pos,
                    leaf_radius + radius_offset - 1,
                    0,
                    double_trunk,
                );
            }
            FoliagePlacer::DarkOak { .. } => {
                let at = attachment.pos + IVec3::Y * offset;
                if double_trunk {
                    self.place_leaves_row(cx, rng, at, leaf_radius + 2, -1, true);
                    self.place_leaves_row(cx, rng, at, leaf_radius + 3, 0, true);
                    self.place_leaves_row(cx, rng, at, leaf_radius + 2, 1, true);
                    if rng.next_bool() {
                        self.place_leaves_row(cx, rng, at, leaf_radius, 2, true);
                    }
                } else {
                    self.place_leaves_row(cx, rng, at, leaf_radius + 2, -1, false);
                    self.place_leaves_row(cx, rng, at, leaf_radius + 1, 0, false);
                }
            }
            FoliagePlacer::MegaJungle { .. } => {
                let leaf_height = if double_trunk {
                    foliage_height
                } else {
                    1 + rng.next_i32_bound(2)
                };
                for yo in (offset - leaf_height..=offset).rev() {
                    let current_radius = leaf_radius + radius_offset + 1 - yo;
                    self.place_leaves_row(
                        cx,
                        rng,
                        attachment.pos,
                        current_radius,
                        yo,
                        double_trunk,
                    );
                }
            }
            FoliagePlacer::MegaPine { .. } => {
                let foliage_pos = attachment.pos;
                let mut previous_radius = 0;
                for yy in (foliage_pos.y - foliage_height + offset)..=(foliage_pos.y + offset) {
                    let yo = foliage_pos.y - yy;
                    let smooth_radius = leaf_radius
                        + radius_offset
                        + (yo as f32 / foliage_height as f32 * 3.5).floor() as i32;
                    let jagged_radius =
                        if yo > 0 && smooth_radius == previous_radius && (yy & 1) == 0 {
                            smooth_radius + 1
                        } else {
                            smooth_radius
                        };
                    self.place_leaves_row(
                        cx,
                        rng,
                        BlockPos::new(foliage_pos.x, yy, foliage_pos.z),
                        jagged_radius,
                        0,
                        double_trunk,
                    );
                    previous_radius = smooth_radius;
                }
            }
            FoliagePlacer::RandomSpread {
                leaf_placement_attempts,
                ..
            } => {
                let origin = attachment.pos;
                for _ in 0..leaf_placement_attempts.0 {
                    let at = origin
                        + IVec3::new(
                            rng.next_i32_bound(leaf_radius) - rng.next_i32_bound(leaf_radius),
                            rng.next_i32_bound(foliage_height) - rng.next_i32_bound(foliage_height),
                            rng.next_i32_bound(leaf_radius) - rng.next_i32_bound(leaf_radius),
                        );
                    self.try_place_leaf(cx, rng, at);
                }
            }
            FoliagePlacer::Cherry {
                hanging_leaves_chance,
                hanging_leaves_extension_chance,
                ..
            } => {
                let foliage_pos = attachment.pos + IVec3::Y * offset;
                let current_radius = leaf_radius + radius_offset - 1;
                self.place_leaves_row(
                    cx,
                    rng,
                    foliage_pos,
                    current_radius - 2,
                    foliage_height - 3,
                    double_trunk,
                );
                self.place_leaves_row(
                    cx,
                    rng,
                    foliage_pos,
                    current_radius - 1,
                    foliage_height - 4,
                    double_trunk,
                );
                for y in (0..=foliage_height - 5).rev() {
                    self.place_leaves_row(cx, rng, foliage_pos, current_radius, y, double_trunk);
                }
                self.place_leaves_row_with_hanging_leaves_below(
                    cx,
                    rng,
                    foliage_pos,
                    current_radius,
                    -1,
                    double_trunk,
                    hanging_leaves_chance.0 as f32,
                    hanging_leaves_extension_chance.0 as f32,
                );
                self.place_leaves_row_with_hanging_leaves_below(
                    cx,
                    rng,
                    foliage_pos,
                    current_radius - 1,
                    -2,
                    double_trunk,
                    hanging_leaves_chance.0 as f32,
                    hanging_leaves_extension_chance.0 as f32,
                );
            }
            FoliagePlacer::Poplar {
                side_hole_chance, ..
            } => {
                let foliage_pos = attachment.pos + IVec3::Y * offset;
                let current_radius = leaf_radius + radius_offset - 1;
                let flip = rng.next_bool();
                let rows = [
                    (current_radius - 2, foliage_height - 1),
                    (current_radius - 1, foliage_height - 2),
                    (current_radius - 1, foliage_height - 3),
                ]
                .into_iter()
                .chain((1..=foliage_height - 4).rev().map(|y| (current_radius, y)));
                for (radius, y) in rows {
                    self.poplar_row(
                        cx,
                        rng,
                        side_hole_chance,
                        foliage_pos,
                        radius,
                        y,
                        double_trunk,
                        foliage_height,
                        flip,
                    );
                }
                self.replace_leaves_with_log(
                    cx,
                    rng,
                    foliage_pos,
                    current_radius,
                    foliage_height - 4,
                    double_trunk,
                    foliage_height,
                    flip,
                );
                self.poplar_row(
                    cx,
                    rng,
                    side_hole_chance,
                    foliage_pos,
                    current_radius - 1,
                    0,
                    double_trunk,
                    foliage_height,
                    flip,
                );
                self.poplar_row(
                    cx,
                    rng,
                    side_hole_chance,
                    foliage_pos,
                    (current_radius - 2).clamp(1, 2),
                    -1,
                    double_trunk,
                    foliage_height,
                    flip,
                );
            }
        }
    }

    /// The skip predicate is asked about **every** cell of the row, the ones it
    /// skips included, and blob, bush and cherry draw inside it.
    fn place_leaves_row<W: WorldGenVolume>(
        &self,
        cx: &mut TreeContext<'_, W>,
        rng: &mut XoroshiroRandom,
        origin: BlockPos,
        current_radius: i32,
        y: i32,
        double_trunk: bool,
    ) {
        let offset = i32::from(double_trunk);
        for dx in -current_radius..=current_radius + offset {
            for dz in -current_radius..=current_radius + offset {
                if !self.should_skip_location_signed(rng, dx, y, dz, current_radius, double_trunk) {
                    self.try_place_leaf(cx, rng, origin + IVec3::new(dx, y, dz));
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn place_leaves_row_with_hanging_leaves_below<W: WorldGenVolume>(
        &self,
        cx: &mut TreeContext<'_, W>,
        rng: &mut XoroshiroRandom,
        origin: BlockPos,
        current_radius: i32,
        y: i32,
        double_trunk: bool,
        hanging_leaves_chance: f32,
        hanging_leaves_extension_chance: f32,
    ) {
        self.place_leaves_row(cx, rng, origin, current_radius, y, double_trunk);
        let offset = i32::from(double_trunk);
        let log_pos = origin - IVec3::Y;

        for along_edge in Direction::HORIZONTAL {
            let to_edge = along_edge.clockwise();
            let offset_to_edge = if to_edge.is_positive() {
                current_radius + offset
            } else {
                current_radius
            };
            let mut at = origin
                + IVec3::new(0, y - 1, 0)
                + to_edge.normal() * offset_to_edge
                + along_edge.normal() * -current_radius;
            let mut offset_along_edge = -current_radius;

            while offset_along_edge < current_radius + offset {
                if cx.is_leaf_set(at + IVec3::Y)
                    && self.try_place_extension(cx, rng, hanging_leaves_chance, log_pos, at)
                {
                    self.try_place_extension(
                        cx,
                        rng,
                        hanging_leaves_extension_chance,
                        log_pos,
                        at - IVec3::Y,
                    );
                }
                offset_along_edge += 1;
                at += along_edge.normal();
            }
        }
    }

    fn should_skip_location_signed(
        &self,
        rng: &mut XoroshiroRandom,
        dx: i32,
        y: i32,
        dz: i32,
        current_radius: i32,
        double_trunk: bool,
    ) -> bool {
        if matches!(self.0, FoliagePlacer::DarkOak { .. })
            && y == 0
            && double_trunk
            && !(dx != -current_radius && dx < current_radius)
            && !(dz != -current_radius && dz < current_radius)
        {
            return true;
        }
        let (min_dx, min_dz) = if double_trunk {
            (dx.abs().min((dx - 1).abs()), dz.abs().min((dz - 1).abs()))
        } else {
            (dx.abs(), dz.abs())
        };
        self.should_skip_location(rng, min_dx, y, min_dz, current_radius, double_trunk)
    }

    fn should_skip_location(
        &self,
        rng: &mut XoroshiroRandom,
        dx: i32,
        y: i32,
        dz: i32,
        current_radius: i32,
        double_trunk: bool,
    ) -> bool {
        match &self.0 {
            FoliagePlacer::Blob { .. } => {
                dx == current_radius
                    && dz == current_radius
                    && (rng.next_i32_bound(2) == 0 || y == 0)
            }
            FoliagePlacer::Bush { .. } => {
                dx == current_radius && dz == current_radius && rng.next_i32_bound(2) == 0
            }
            FoliagePlacer::Fancy { .. } => {
                let fx = dx as f32 + 0.5;
                let fz = dz as f32 + 0.5;
                fx * fx + fz * fz > (current_radius * current_radius) as f32
            }
            FoliagePlacer::Spruce { .. } | FoliagePlacer::Pine { .. } => {
                dx == current_radius && dz == current_radius && current_radius > 0
            }
            FoliagePlacer::Acacia { .. } => {
                if y == 0 {
                    (dx > 1 || dz > 1) && dx != 0 && dz != 0
                } else {
                    dx == current_radius && dz == current_radius && current_radius > 0
                }
            }
            FoliagePlacer::DarkOak { .. } => {
                if y == -1 && !double_trunk {
                    dx == current_radius && dz == current_radius
                } else if y == 1 {
                    dx + dz > current_radius * 2 - 2
                } else {
                    false
                }
            }
            FoliagePlacer::MegaJungle { .. } | FoliagePlacer::MegaPine { .. } => {
                dx + dz >= 7 || dx * dx + dz * dz > current_radius * current_radius
            }
            FoliagePlacer::RandomSpread { .. } => false,
            FoliagePlacer::Cherry {
                wide_bottom_layer_hole_chance,
                corner_hole_chance,
                ..
            } => {
                if y == -1
                    && (dx == current_radius || dz == current_radius)
                    && rng.next_f32() < wide_bottom_layer_hole_chance.0 as f32
                {
                    return true;
                }
                let corner = dx == current_radius && dz == current_radius;
                if current_radius > 2 {
                    corner
                        || dx + dz > current_radius * 2 - 2
                            && rng.next_f32() < corner_hole_chance.0 as f32
                } else {
                    corner && rng.next_f32() < corner_hole_chance.0 as f32
                }
            }
            FoliagePlacer::Poplar { .. } => {
                unreachable!("the poplar rhombus needs the row's own height and flip")
            }
        }
    }
    fn try_place_leaf<W: WorldGenVolume>(
        &self,
        cx: &mut TreeContext<'_, W>,
        rng: &mut XoroshiroRandom,
        at: BlockPos,
    ) -> bool {
        if cx.is_persistent(at) || !cx.is_valid_tree_pos(at) {
            return false;
        }
        let state = cx.foliage_provider.state(cx.volume, rng, at);
        let state = cx.states.with_waterlogged(state, cx.is_water_source(at));
        cx.set_leaf(at, state);
        true
    }

    fn try_place_extension<W: WorldGenVolume>(
        &self,
        cx: &mut TreeContext<'_, W>,
        rng: &mut XoroshiroRandom,
        chance: f32,
        log_pos: BlockPos,
        at: BlockPos,
    ) -> bool {
        if dist_manhattan(*at, *log_pos) >= 7 {
            return false;
        }
        if rng.next_f32() > chance {
            return false;
        }
        self.try_place_leaf(cx, rng, at)
    }

    /// The poplar's own row: its skip test needs the row's height and the
    /// tree's rhombus flip, neither of which the shared predicate carries.
    #[allow(clippy::too_many_arguments)]
    fn poplar_row<W: WorldGenVolume>(
        &self,
        cx: &mut TreeContext<'_, W>,
        rng: &mut XoroshiroRandom,
        side_hole_chance: &UnitFloat,
        origin: BlockPos,
        current_radius: i32,
        y: i32,
        double_trunk: bool,
        foliage_height: i32,
        flip_rhombus_shape: bool,
    ) {
        let offset = i32::from(double_trunk);

        for dx in -current_radius..=current_radius + offset {
            for dz in -current_radius..=current_radius + offset {
                let partial = should_row_be_partial_rhombus_shape(foliage_height, y);
                let corner_blocks = corner_blocks_to_cut_for_rhombus_shape(
                    dx,
                    dz,
                    current_radius,
                    partial,
                    flip_rhombus_shape,
                );
                let abs_dx = dx.abs();
                let abs_dz = dz.abs();
                let rhombus_edge = abs_dx == current_radius || abs_dz == current_radius;
                let skip = if partial && rhombus_edge {
                    true
                } else {
                    let additional = i32::from(rng.next_f32() <= side_hole_chance.0 as f32);
                    !is_within_rhombus_shape(
                        current_radius,
                        abs_dx,
                        abs_dz,
                        corner_blocks,
                        additional,
                    )
                };
                if !skip {
                    self.try_place_leaf(cx, rng, origin + IVec3::new(dx, y, dz));
                }
            }
        }
    }

    /// The poplar's log pass. The foliage provider is sampled to compare
    /// against the block already there, so it draws whether or not a log lands.
    #[allow(clippy::too_many_arguments)]
    fn replace_leaves_with_log<W: WorldGenVolume>(
        &self,
        cx: &mut TreeContext<'_, W>,
        rng: &mut XoroshiroRandom,
        origin: BlockPos,
        current_radius: i32,
        y: i32,
        double_trunk: bool,
        foliage_height: i32,
        flip_rhombus_shape: bool,
    ) {
        let offset = i32::from(double_trunk);
        let partial = should_row_be_partial_rhombus_shape(foliage_height, y);

        for dx in -current_radius..=current_radius + offset {
            for dz in -current_radius..=current_radius + offset {
                let abs_dz = dz.abs();
                let abs_dx = dx.abs();
                let corner_blocks = corner_blocks_to_cut_for_rhombus_shape(
                    dx,
                    dz,
                    current_radius,
                    partial,
                    flip_rhombus_shape,
                );
                let arm = abs_dz == 0 && current_radius - abs_dx >= 4
                    || abs_dx == 0 && current_radius - abs_dz >= 4;
                if !(is_within_rhombus_shape(current_radius, abs_dx, abs_dz, corner_blocks, 2)
                    && arm)
                {
                    continue;
                }
                let at = origin + IVec3::new(dx, y, dz);
                let leaf = cx.foliage_provider.state(cx.volume, rng, at);
                if cx.get(at) != leaf {
                    continue;
                }
                let log = cx.trunk_provider.state(cx.volume, rng, at);
                let axis = if abs_dz == 0 { Axis::X } else { Axis::Z };
                let log = cx.states.with_axis(log, axis);
                cx.set_leaf(at, log);
            }
        }
    }
}

fn should_row_be_partial_rhombus_shape(foliage_height: i32, y: i32) -> bool {
    foliage_height - 1 == y || foliage_height - 2 == y
}

fn corner_blocks_to_cut_for_rhombus_shape(
    dx: i32,
    dz: i32,
    current_radius: i32,
    partial: bool,
    flip_rhombus_shape: bool,
) -> i32 {
    let small_corner = if flip_rhombus_shape {
        dx > 0 && dz > 0 || dz < 0 && dx < 0
    } else {
        dx > 0 && dz < 0 || dz > 0 && dx < 0
    };
    if small_corner {
        current_radius - 1
    } else if partial {
        current_radius + 1
    } else {
        current_radius
    }
}

fn is_within_rhombus_shape(
    current_radius: i32,
    abs_dx: i32,
    abs_dz: i32,
    corner_blocks: i32,
    additional_side_removal: i32,
) -> bool {
    abs_dx + abs_dz <= current_radius * 2 - (corner_blocks + additional_side_removal)
}

#[cfg(test)]
mod tests {
    use super::super::trunk::harness::*;
    use super::*;
    use mcrs_minecraft_random::xoroshiro::XoroshiroRandom;

    fn foliage(placer: &str, radius: i32, offset: i32, rest: &str) -> Foliage {
        let json = format!(
            r#"{{"type":"minecraft:{placer}_foliage_placer","radius":{radius},"offset":{offset}{rest}}}"#
        );
        Foliage(serde_json::from_str(&json).unwrap())
    }

    /// One case per foliage type, parameters taken from the shipped feature
    /// that uses it. `tree_height` is the trunk the crown was told about.
    fn cases() -> Vec<(&'static str, Foliage, i32, bool)> {
        vec![
            ("blob", foliage("blob", 2, 0, r#","height":3"#), 5, false),
            ("bush", foliage("bush", 2, 0, r#","height":3"#), 5, false),
            ("fancy", foliage("fancy", 2, 0, r#","height":4"#), 5, false),
            (
                "spruce",
                foliage(
                    "spruce",
                    2,
                    1,
                    &format!(r#","trunk_height":{}"#, uniform(1, 2)),
                ),
                10,
                false,
            ),
            (
                "pine",
                foliage("pine", 1, 1, &format!(r#","height":{}"#, uniform(3, 4))),
                10,
                false,
            ),
            ("acacia", foliage("acacia", 2, 0, ""), 6, false),
            ("dark_oak", foliage("dark_oak", 0, 0, ""), 6, true),
            (
                "mega_jungle",
                foliage("jungle", 2, 0, r#","height":2"#),
                12,
                true,
            ),
            (
                "mega_pine",
                foliage(
                    "mega_pine",
                    0,
                    0,
                    &format!(r#","crown_height":{}"#, uniform(3, 7)),
                ),
                18,
                true,
            ),
            (
                "random_spread",
                foliage(
                    "random_spread",
                    3,
                    0,
                    r#","foliage_height":4,"leaf_placement_attempts":40"#,
                ),
                8,
                false,
            ),
            (
                "cherry",
                foliage(
                    "cherry",
                    4,
                    0,
                    &format!(
                        r#","height":{},"wide_bottom_layer_hole_chance":0.11,"corner_hole_chance":0.25,"hanging_leaves_chance":0.55,"hanging_leaves_extension_chance":0.4"#,
                        uniform(7, 9)
                    ),
                ),
                7,
                false,
            ),
            (
                "poplar",
                foliage(
                    "poplar",
                    4,
                    0,
                    &format!(r#","height":{},"side_hole_chance":0.2"#, uniform(6, 8)),
                ),
                9,
                false,
            ),
        ]
    }

    fn pin_of(placer: &Foliage, tree_height: i32, double_trunk: bool) -> Pin {
        run(42, |cx, rng| {
            let foliage_height = placer.foliage_height(rng, tree_height);
            let leaf_radius = placer.foliage_radius(rng, tree_height - foliage_height);
            let attachment = FoliageAttachment::new(BlockPos::new(8, 64, 8), 0, double_trunk);
            placer.create_foliage(cx, rng, attachment, foliage_height, leaf_radius);
        })
    }

    #[test]
    #[ignore = "prints the table the pin test asserts against"]
    fn print_foliage_pins() {
        for (name, placer, tree_height, double_trunk) in cases() {
            let pin = pin_of(&placer, tree_height, double_trunk);
            println!(
                "(\"{name}\", {}, {:#x}, {}, {}),",
                pin.writes, pin.digest, pin.rng_after[0], pin.rng_after[1]
            );
        }
    }

    #[test]
    fn every_foliage_placer_writes_and_draws_what_it_did() {
        assert_eq!(
            cases().len(),
            FOLIAGE_PINS.len(),
            "a foliage placer has no pin row, and zip would skip it"
        );
        for ((name, placer, tree_height, double_trunk), (pin_name, writes, digest, lo, hi)) in
            cases().iter().zip(FOLIAGE_PINS)
        {
            assert_eq!(name, pin_name);
            assert_eq!(
                pin_of(placer, *tree_height, *double_trunk),
                Pin {
                    writes: *writes,
                    digest: *digest,
                    rng_after: [*lo, *hi],
                },
                "{name}"
            );
        }
    }

    /// `placeLeavesRow` asks the skip predicate about every cell of the row,
    /// so blob spends a draw on each corner — including the corners the answer
    /// then refuses to fill.
    #[test]
    fn the_skip_predicate_draws_on_the_cells_it_skips() {
        let placer = foliage("blob", 1, 0, r#","height":0"#);
        let mut rng = XoroshiroRandom::new(5);
        let mut drew = |dx: i32, dz: i32| {
            let before = rng.clone().next_i64();
            let skip = placer.should_skip_location_signed(&mut rng, dx, 1, dz, 1, false);
            (skip, before != rng.clone().next_i64())
        };

        let mut skipped = 0;
        for (dx, dz) in [(-1, -1), (-1, 1), (1, -1), (1, 1)] {
            let (skip, drew_here) = drew(dx, dz);
            assert!(drew_here, "the corner ({dx}, {dz}) must draw");
            skipped += usize::from(skip);
        }
        assert!(skipped > 0, "a corner the predicate skips still drew");

        for (dx, dz) in [(0, 0), (1, 0), (0, -1)] {
            assert!(
                !drew(dx, dz).1,
                "({dx}, {dz}) is not a corner and must not draw"
            );
        }
    }

    /// `dark_oak` overrides the *signed* predicate: at `y == 0` under a double
    /// trunk it keeps the four outer corners rather than asking further.
    #[test]
    fn dark_oak_keeps_its_double_trunk_corners() {
        let placer = foliage("dark_oak", 0, 0, "");
        let mut rng = XoroshiroRandom::new(1);
        assert!(placer.should_skip_location_signed(&mut rng, -2, 0, -2, 2, true));
        assert!(placer.should_skip_location_signed(&mut rng, 3, 0, 3, 2, true));
        assert!(!placer.should_skip_location_signed(&mut rng, 0, 0, 0, 2, true));
    }

    const FOLIAGE_PINS: &[(&str, usize, u64, i64, i64)] = &[
        (
            "blob",
            60,
            0xfbea357879acb37f,
            -3066404182989085885,
            745457546272312399,
        ),
        (
            "bush",
            157,
            0xb3c9bf63146fd866,
            -3066404182989085885,
            745457546272312399,
        ),
        (
            "fancy",
            73,
            0xf5e63a77d813a3b0,
            -4695948378737616609,
            7341713790291473579,
        ),
        (
            "spruce",
            111,
            0x98f419d8e1b3333e,
            -7542733514721318211,
            4888889476139319686,
        ),
        (
            "pine",
            32,
            0xa8e40b3184493065,
            -7542733514721318211,
            4888889476139319686,
        ),
        (
            "acacia",
            30,
            0x382cb66bed781e2,
            -4695948378737616609,
            7341713790291473579,
        ),
        (
            "dark_oak",
            119,
            0x5e07e34e943710a0,
            7341713790291473579,
            -7542733514721318211,
        ),
        (
            "mega_jungle",
            208,
            0x17b9d7f337b89b65,
            -4695948378737616609,
            7341713790291473579,
        ),
        (
            "mega_pine",
            112,
            0x9dbf0e3ece82c095,
            7341713790291473579,
            -7542733514721318211,
        ),
        (
            "random_spread",
            34,
            0x2b00ed3eb25def67,
            8416111302833757510,
            -5134745859768971629,
        ),
        (
            "cherry",
            294,
            0x68673acb05e939f6,
            -9161065153311613817,
            -760078507025830770,
        ),
        (
            "poplar",
            129,
            0x99327487510fa011,
            -3254201749868996534,
            -9191476076367467042,
        ),
    ];
}
