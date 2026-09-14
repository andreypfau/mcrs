use crate::SampleGrid;
use crate::proto::{DensityFunctionHolder, ProtoDensityFunction};
use crate::structure::{Projection, TerrainAdaptation};
use mcrs_minecraft_core::ResourceLocation;
use mcrs_voxel_math::mth::{fast_inv_sqrt, floor_div};
use mcrs_voxel_math::{BlockPos, BoundingBox, ColumnPos};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::LazyLock;

pub const KERNEL_RADIUS: i32 = 12;
const KERNEL_SIZE: i32 = KERNEL_RADIUS * 2;
pub const KERNEL_LEN: usize = (KERNEL_SIZE * KERNEL_SIZE * KERNEL_SIZE) as usize;
const PIECE_REACH: i32 = 12;
const AFFECTED_MARGIN: i32 = 24;

pub static KERNEL: LazyLock<[f32; KERNEL_LEN]> = LazyLock::new(|| {
    let mut kernel = [0.0f32; KERNEL_LEN];
    for zi in 0..KERNEL_SIZE {
        for xi in 0..KERNEL_SIZE {
            for yi in 0..KERNEL_SIZE {
                let dx = (xi - KERNEL_RADIUS) as f64;
                let dy = (yi - KERNEL_RADIUS) as f64 + 0.5;
                let dz = (zi - KERNEL_RADIUS) as f64;
                let distance_sq = dx * dx + dy * dy + dz * dz;
                kernel[kernel_index(xi, yi, zi)] =
                    std::f64::consts::E.powf(-distance_sq / 16.0) as f32;
            }
        }
    }
    kernel
});

#[inline]
fn kernel_index(xi: i32, yi: i32, zi: i32) -> usize {
    ((zi * KERNEL_SIZE + xi) * KERNEL_SIZE + yi) as usize
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rigid {
    pub bounds: BoundingBox,
    pub adaptation: TerrainAdaptation,
    pub ground_level_delta: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct JunctionPoint {
    pub x: i32,
    pub ground_y: i32,
    pub z: i32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BeardPiece<J> {
    pub bounds: BoundingBox,
    pub projection: Projection,
    pub ground_level_delta: i32,
    pub junctions: J,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Beard {
    pub rigids: Vec<Rigid>,
    pub junctions: Vec<JunctionPoint>,
    pub affected: Option<BoundingBox>,
}

impl Beard {
    /// `pieces` in start order, then piece order within each start, each paired
    /// with its structure's adaptation.
    pub fn collect<J>(
        chunk: ColumnPos,
        pieces: impl IntoIterator<Item = (TerrainAdaptation, BeardPiece<J>)>,
    ) -> Self
    where
        J: IntoIterator<Item = JunctionPoint>,
    {
        let reach_min_x = chunk.min_block_x() - PIECE_REACH;
        let reach_min_z = chunk.min_block_z() - PIECE_REACH;
        let reach_max_x = chunk.min_block_x() + 15 + PIECE_REACH;
        let reach_max_z = chunk.min_block_z() + 15 + PIECE_REACH;

        let mut rigids = Vec::new();
        let mut junctions = Vec::new();
        let mut touched: Option<BoundingBox> = None;
        let mut include = |bounds: BoundingBox| {
            touched = Some(touched.map_or(bounds, |t| t.union(bounds)));
        };

        for (adaptation, piece) in pieces {
            if adaptation == TerrainAdaptation::None {
                continue;
            }
            let BeardPiece {
                bounds,
                projection,
                ground_level_delta,
                junctions: piece_junctions,
            } = piece;
            let close = bounds.max.x >= reach_min_x
                && bounds.min.x <= reach_max_x
                && bounds.max.z >= reach_min_z
                && bounds.min.z <= reach_max_z;
            if !close {
                continue;
            }
            if projection == Projection::Rigid {
                rigids.push(Rigid {
                    bounds,
                    adaptation,
                    ground_level_delta,
                });
                include(bounds);
            }
            for junction in piece_junctions {
                if junction.x > reach_min_x
                    && junction.z > reach_min_z
                    && junction.x < reach_max_x
                    && junction.z < reach_max_z
                {
                    junctions.push(junction);
                    include(BoundingBox::point(BlockPos::new(
                        junction.x,
                        junction.ground_y,
                        junction.z,
                    )));
                }
            }
        }

        Self {
            rigids,
            junctions,
            affected: touched.map(|bounds| bounds.inflated(AFFECTED_MARGIN)),
        }
    }

    pub fn intersects(&self, grid: &SampleGrid) -> bool {
        self.affected.is_some_and(|affected| {
            affected.intersects(BoundingBox {
                min: grid.min_block().into(),
                max: grid.max_block().into(),
            })
        })
    }

    pub fn sample(&self, x: i32, y: i32, z: i32) -> f32 {
        match self.affected {
            Some(affected) if affected.contains(BoundingBox::point(BlockPos::new(x, y, z))) => {
                self.sample_unchecked(&KERNEL, x, y, z)
            }
            _ => 0.0,
        }
    }

    /// Adds the term into `out`, indexed like `grid`, writing only the lattice
    /// points of the affected box.
    pub fn fill(&self, grid: &SampleGrid, out: &mut [f32]) {
        assert_eq!(out.len(), grid.len(), "the buffer must cover the grid");
        let Some(affected) = self.affected.filter(|_| self.intersects(grid)) else {
            return;
        };
        let (min, step, size) = (grid.min_block(), grid.step_block(), grid.size());
        let lo_x = floor_div(0.max(affected.min.x - min.x), step.x);
        let lo_y = floor_div(0.max(affected.min.y - min.y), step.y);
        let lo_z = floor_div(0.max(affected.min.z - min.z), step.z);
        let hi_x = (size.x - 1).min(floor_div(affected.max.x - min.x, step.x));
        let hi_y = (size.y - 1).min(floor_div(affected.max.y - min.y, step.y));
        let hi_z = (size.z - 1).min(floor_div(affected.max.z - min.z, step.z));

        let kernel = &*KERNEL;
        for z in lo_z..=hi_z {
            let block_z = grid.block_z(z);
            for x in lo_x..=hi_x {
                let block_x = grid.block_x(x);
                for y in lo_y..=hi_y {
                    out[grid.index_unchecked(x, y, z)] +=
                        self.sample_unchecked(kernel, block_x, grid.block_y(y), block_z);
                }
            }
        }
    }

    fn sample_unchecked(&self, kernel: &[f32; KERNEL_LEN], x: i32, y: i32, z: i32) -> f32 {
        let mut noise = 0.0f32;
        for rigid in &self.rigids {
            let bounds = rigid.bounds;
            let dx = 0.max((bounds.min.x - x).max(x - bounds.max.x));
            let dz = 0.max((bounds.min.z - z).max(z - bounds.max.z));
            let ground_y = bounds.min.y + rigid.ground_level_delta;
            let dy_to_ground = y - ground_y;
            noise += match rigid.adaptation {
                TerrainAdaptation::None => 0.0,
                TerrainAdaptation::Bury => {
                    bury_contribution(dx as f32, dy_to_ground as f32 / 2.0, dz as f32)
                }
                TerrainAdaptation::BeardThin => {
                    beard_contribution(kernel, dx, dy_to_ground, dz, dy_to_ground) * 0.8
                }
                TerrainAdaptation::BeardBox => {
                    let dy = 0.max((ground_y - y).max(y - bounds.max.y));
                    beard_contribution(kernel, dx, dy, dz, dy_to_ground) * 0.8
                }
                TerrainAdaptation::Encapsulate => {
                    let dy = 0.max((bounds.min.y - y).max(y - bounds.max.y));
                    bury_contribution(dx as f32 / 2.0, dy as f32 / 2.0, dz as f32 / 2.0) * 0.8
                }
            };
        }
        for junction in &self.junctions {
            let dy = y - junction.ground_y;
            noise += beard_contribution(kernel, x - junction.x, dy, z - junction.z, dy) * 0.4;
        }
        noise
    }
}

#[inline]
fn bury_contribution(dx: f32, dy: f32, dz: f32) -> f32 {
    let distance_sq = dx * dx + dy * dy + dz * dz;
    if distance_sq >= 36.0 {
        0.0
    } else {
        1.0 - (distance_sq as f64).sqrt() as f32 / 6.0
    }
}

#[inline]
fn beard_contribution(
    kernel: &[f32; KERNEL_LEN],
    dx: i32,
    dy: i32,
    dz: i32,
    y_to_ground: i32,
) -> f32 {
    let (xi, yi, zi) = (dx + KERNEL_RADIUS, dy + KERNEL_RADIUS, dz + KERNEL_RADIUS);
    let in_range = |i: i32| (0..KERNEL_SIZE).contains(&i);
    if !(in_range(xi) && in_range(yi) && in_range(zi)) {
        return 0.0;
    }
    let dy_offset = y_to_ground as f32 + 0.5;
    let (dx, dz) = (dx as f32, dz as f32);
    let distance_sq = dx * dx + dy_offset * dy_offset + dz * dz;
    let value = -dy_offset * fast_inv_sqrt((distance_sq / 2.0) as f64) as f32 / 2.0;
    value * kernel[kernel_index(xi, yi, zi)]
}

/// Where `minecraft:beardifier` sits in a `final_density` graph. The fill adds
/// the term after evaluating the graph, which equals the graph only when the
/// beardifier is an operand of the outermost `add` and nowhere else.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BeardifierPlacement {
    Absent,
    RootAddend,
    Misplaced,
}

pub fn beardifier_placement(
    final_density: &DensityFunctionHolder,
    registry: &BTreeMap<ResourceLocation, DensityFunctionHolder>,
) -> BeardifierPlacement {
    let root = resolve(final_density, registry);
    if let DensityFunctionHolder::Owned(function) = root
        && let ProtoDensityFunction::Add(add) = &**function
    {
        for (addend, rest) in [(&add.left, &add.right), (&add.right, &add.left)] {
            if is_beardifier(addend, registry) && !contains_beardifier(rest, registry) {
                return BeardifierPlacement::RootAddend;
            }
        }
    }
    if contains_beardifier(root, registry) {
        BeardifierPlacement::Misplaced
    } else {
        BeardifierPlacement::Absent
    }
}

fn resolve<'a>(
    mut holder: &'a DensityFunctionHolder,
    registry: &'a BTreeMap<ResourceLocation, DensityFunctionHolder>,
) -> &'a DensityFunctionHolder {
    for _ in 0..=registry.len() {
        let DensityFunctionHolder::Reference(id) = holder else {
            break;
        };
        match registry.get(id) {
            Some(target) => holder = target,
            None => break,
        }
    }
    holder
}

fn is_beardifier(
    holder: &DensityFunctionHolder,
    registry: &BTreeMap<ResourceLocation, DensityFunctionHolder>,
) -> bool {
    matches!(
        resolve(holder, registry),
        DensityFunctionHolder::Owned(function)
            if matches!(**function, ProtoDensityFunction::Beardifier)
    )
}

fn contains_beardifier(
    holder: &DensityFunctionHolder,
    registry: &BTreeMap<ResourceLocation, DensityFunctionHolder>,
) -> bool {
    fn walk(
        holder: &DensityFunctionHolder,
        registry: &BTreeMap<ResourceLocation, DensityFunctionHolder>,
        seen: &mut BTreeSet<ResourceLocation>,
    ) -> bool {
        match holder {
            DensityFunctionHolder::Value(_) => false,
            DensityFunctionHolder::Reference(id) => {
                seen.insert(id.clone())
                    && registry
                        .get(id)
                        .is_some_and(|target| walk(target, registry, seen))
            }
            DensityFunctionHolder::Owned(function) => {
                if matches!(**function, ProtoDensityFunction::Beardifier) {
                    return true;
                }
                let mut found = false;
                function.visit_children(&mut |child| {
                    found = found || walk(child, registry, seen);
                });
                found
            }
        }
    }
    walk(holder, registry, &mut BTreeSet::new())
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_math::IVec3;

    const CHUNK: ColumnPos = ColumnPos::new(0, 0);

    fn cube(min: [i32; 3], max: [i32; 3]) -> BoundingBox {
        BoundingBox {
            min: BlockPos::new(min[0], min[1], min[2]),
            max: BlockPos::new(max[0], max[1], max[2]),
        }
    }

    fn point(x: i32, ground_y: i32, z: i32) -> JunctionPoint {
        JunctionPoint { x, ground_y, z }
    }

    fn jigsaw(
        bounds: BoundingBox,
        projection: Projection,
        junctions: Vec<JunctionPoint>,
    ) -> BeardPiece<Vec<JunctionPoint>> {
        BeardPiece {
            bounds,
            projection,
            ground_level_delta: 1,
            junctions,
        }
    }

    fn rigid(bounds: BoundingBox) -> BeardPiece<Vec<JunctionPoint>> {
        BeardPiece {
            bounds,
            projection: Projection::Rigid,
            ground_level_delta: 0,
            junctions: Vec::new(),
        }
    }

    fn collect(pieces: Vec<(TerrainAdaptation, BeardPiece<Vec<JunctionPoint>>)>) -> Beard {
        Beard::collect(CHUNK, pieces)
    }

    #[test]
    fn a_piece_counts_up_to_twelve_blocks_from_the_footprint() {
        let thin = TerrainAdaptation::BeardThin;
        let other = |bounds| (thin, rigid(bounds));
        let beard = collect(vec![
            other(cube([-20, 0, 0], [-12, 5, 3])),
            other(cube([-20, 0, 0], [-13, 5, 3])),
            other(cube([27, 0, 0], [30, 5, 3])),
            other(cube([28, 0, 0], [30, 5, 3])),
            other(cube([0, 0, -20], [3, 5, -12])),
            other(cube([0, 0, -20], [3, 5, -13])),
            other(cube([0, 0, 27], [3, 5, 30])),
            other(cube([0, 0, 28], [3, 5, 30])),
        ]);
        let kept: Vec<_> = beard.rigids.iter().map(|rigid| rigid.bounds).collect();
        assert_eq!(
            kept,
            vec![
                cube([-20, 0, 0], [-12, 5, 3]),
                cube([27, 0, 0], [30, 5, 3]),
                cube([0, 0, -20], [3, 5, -12]),
                cube([0, 0, 27], [3, 5, 30]),
            ]
        );
    }

    #[test]
    fn junctions_count_only_strictly_inside_the_inflated_footprint() {
        let beard = collect(vec![(
            TerrainAdaptation::BeardThin,
            jigsaw(
                cube([0, 60, 0], [8, 70, 8]),
                Projection::Rigid,
                vec![
                    point(-12, 64, 5),
                    point(-11, 64, 5),
                    point(27, 64, 5),
                    point(26, 64, 5),
                    point(5, 64, -12),
                    point(5, 64, -11),
                    point(5, 64, 27),
                    point(5, 64, 26),
                ],
            ),
        )]);
        assert_eq!(
            beard.junctions,
            [
                point(-11, 64, 5),
                point(26, 64, 5),
                point(5, 64, -11),
                point(5, 64, 26)
            ]
        );
    }

    #[test]
    fn a_terrain_matching_piece_contributes_only_its_junctions() {
        let beard = collect(vec![(
            TerrainAdaptation::BeardThin,
            jigsaw(
                cube([0, 60, 0], [8, 70, 8]),
                Projection::TerrainMatching,
                vec![point(3, 64, 4)],
            ),
        )]);
        assert!(beard.rigids.is_empty());
        assert_eq!(beard.junctions, [point(3, 64, 4)]);
        assert_eq!(beard.affected, Some(cube([-21, 40, -20], [27, 88, 28])));
    }

    #[test]
    fn the_affected_box_is_the_union_inflated_by_twenty_four() {
        let beard = collect(vec![
            (
                TerrainAdaptation::Bury,
                jigsaw(
                    cube([0, 60, 0], [8, 70, 8]),
                    Projection::Rigid,
                    vec![point(20, 50, -5)],
                ),
            ),
            (
                TerrainAdaptation::Encapsulate,
                rigid(cube([-4, 80, 2], [1, 90, 3])),
            ),
        ]);
        assert_eq!(
            beard.rigids,
            [
                Rigid {
                    bounds: cube([0, 60, 0], [8, 70, 8]),
                    adaptation: TerrainAdaptation::Bury,
                    ground_level_delta: 1,
                },
                Rigid {
                    bounds: cube([-4, 80, 2], [1, 90, 3]),
                    adaptation: TerrainAdaptation::Encapsulate,
                    ground_level_delta: 0,
                },
            ]
        );
        assert_eq!(beard.affected, Some(cube([-28, 26, -29], [44, 114, 32])));
    }

    #[test]
    fn a_structure_that_does_not_adapt_is_skipped() {
        let beard = collect(vec![(
            TerrainAdaptation::None,
            jigsaw(
                cube([0, 60, 0], [8, 70, 8]),
                Projection::Rigid,
                vec![point(3, 64, 4)],
            ),
        )]);
        assert_eq!(beard, Beard::default());
    }

    #[test]
    fn nothing_close_collects_to_no_affected_box() {
        let far = collect(vec![(
            TerrainAdaptation::BeardBox,
            rigid(cube([100, 0, 100], [110, 10, 110])),
        )]);
        assert_eq!(far, Beard::default());
        assert_eq!(collect(Vec::new()).affected, None);

        let grid = SampleGrid::dense(IVec3::new(16, 384, 16), IVec3::new(0, -64, 0));
        assert!(!far.intersects(&grid));
        let mut out = vec![1.5; grid.len()];
        far.fill(&grid, &mut out);
        assert!(out.iter().all(|&v| v == 1.5));
        assert_eq!(far.sample(0, 0, 0), 0.0);
    }

    #[test]
    fn the_footprint_follows_negative_chunk_coordinates() {
        let beard = Beard::collect(
            ColumnPos::new(-1, -2),
            vec![
                (
                    TerrainAdaptation::BeardThin,
                    rigid(cube([-40, 0, -45], [-29, 5, -44])),
                ),
                (
                    TerrainAdaptation::BeardThin,
                    rigid(cube([-40, 0, -45], [-28, 5, -44])),
                ),
            ],
        );
        assert_eq!(beard.rigids.len(), 1);
        assert_eq!(beard.rigids[0].bounds, cube([-40, 0, -45], [-28, 5, -44]));
    }

    #[test]
    fn fill_writes_the_affected_lattice_points_clipped_by_floor_division() {
        let beard = Beard {
            rigids: Vec::new(),
            junctions: Vec::new(),
            affected: Some(cube([5, -60, -3], [9, -50, 100])),
        };
        let grid = SampleGrid::new(
            IVec3::new(5, 49, 5),
            IVec3::new(0, -64, 0),
            IVec3::new(4, 8, 4),
        );
        // Adding the zero term turns -0.0 into +0.0, which marks exactly the written indices.
        let mut out = vec![-0.0f32; grid.len()];
        beard.fill(&grid, &mut out);
        let written: Vec<_> = (0..5)
            .flat_map(|z| (0..5).flat_map(move |x| (0..49).map(move |y| (x, y, z))))
            .filter(|&(x, y, z)| out[grid.index_unchecked(x, y, z)].is_sign_positive())
            .collect();
        let expected: Vec<_> = (0..5)
            .flat_map(|z| (1..=2).flat_map(move |x| (0..=1).map(move |y| (x, y, z))))
            .collect();
        assert_eq!(written, expected);
    }

    #[test]
    fn fill_agrees_with_sample_on_a_stepped_grid() {
        let beard = Beard::collect(
            CHUNK,
            vec![
                (
                    TerrainAdaptation::BeardThin,
                    jigsaw(
                        cube([2, 60, 3], [12, 70, 9]),
                        Projection::Rigid,
                        vec![point(6, 58, 10)],
                    ),
                ),
                (
                    TerrainAdaptation::Encapsulate,
                    rigid(cube([-6, 30, -6], [4, 40, 4])),
                ),
            ],
        );
        let grid = SampleGrid::new(
            IVec3::new(5, 49, 5),
            IVec3::new(-4, -64, -4),
            IVec3::new(4, 8, 4),
        );
        let mut out = vec![0.0f32; grid.len()];
        beard.fill(&grid, &mut out);
        for z in 0..5 {
            for x in 0..5 {
                for y in 0..49 {
                    let (bx, by, bz) = (grid.block_x(x), grid.block_y(y), grid.block_z(z));
                    assert_eq!(
                        out[grid.index_unchecked(x, y, z)].to_bits(),
                        beard.sample(bx, by, bz).to_bits(),
                        "at ({bx}, {by}, {bz})"
                    );
                }
            }
        }
        assert!(out.iter().any(|&v| v != 0.0));
    }

    fn owned(function: ProtoDensityFunction) -> DensityFunctionHolder {
        DensityFunctionHolder::Owned(Box::new(function))
    }

    fn add(left: DensityFunctionHolder, right: DensityFunctionHolder) -> DensityFunctionHolder {
        owned(ProtoDensityFunction::Add(
            crate::proto::args::TwoArgumentFunction { left, right },
        ))
    }

    fn squeeze(input: DensityFunctionHolder) -> DensityFunctionHolder {
        owned(ProtoDensityFunction::Squeeze(
            crate::proto::args::SingleArgumentFunction { input },
        ))
    }

    fn beardifier() -> DensityFunctionHolder {
        owned(ProtoDensityFunction::Beardifier)
    }

    fn terrain() -> DensityFunctionHolder {
        squeeze(DensityFunctionHolder::Value(0.5.into()))
    }

    fn reference(id: &str) -> DensityFunctionHolder {
        DensityFunctionHolder::Reference(ResourceLocation::minecraft(id))
    }

    fn placement(
        root: DensityFunctionHolder,
        registry: &[(&str, DensityFunctionHolder)],
    ) -> BeardifierPlacement {
        let registry = registry
            .iter()
            .map(|(id, holder)| (ResourceLocation::minecraft(id), holder.clone()))
            .collect();
        beardifier_placement(&root, &registry)
    }

    #[test]
    fn a_beardifier_added_at_the_root_is_accepted_on_either_side() {
        assert_eq!(
            placement(add(terrain(), beardifier()), &[]),
            BeardifierPlacement::RootAddend
        );
        assert_eq!(
            placement(add(beardifier(), terrain()), &[]),
            BeardifierPlacement::RootAddend
        );
    }

    #[test]
    fn the_root_and_its_addend_resolve_through_references() {
        let registry = [
            ("final", add(terrain(), reference("beard"))),
            ("beard", beardifier()),
        ];
        assert_eq!(
            placement(reference("final"), &registry),
            BeardifierPlacement::RootAddend
        );
    }

    #[test]
    fn a_beardifier_below_the_root_is_misplaced() {
        assert_eq!(
            placement(squeeze(add(terrain(), beardifier())), &[]),
            BeardifierPlacement::Misplaced
        );
        assert_eq!(
            placement(add(squeeze(beardifier()), terrain()), &[]),
            BeardifierPlacement::Misplaced
        );
        assert_eq!(
            placement(add(beardifier(), beardifier()), &[]),
            BeardifierPlacement::Misplaced
        );
        let registry = [("shared", add(terrain(), beardifier()))];
        assert_eq!(
            placement(add(reference("shared"), beardifier()), &registry),
            BeardifierPlacement::Misplaced
        );
        assert_eq!(placement(beardifier(), &[]), BeardifierPlacement::Misplaced);
    }

    #[test]
    fn a_graph_without_a_beardifier_is_absent_even_through_a_cycle() {
        assert_eq!(placement(terrain(), &[]), BeardifierPlacement::Absent);
        let registry = [
            ("a", squeeze(reference("b"))),
            ("b", squeeze(reference("a"))),
        ];
        assert_eq!(
            placement(add(reference("a"), terrain()), &registry),
            BeardifierPlacement::Absent
        );
    }

    #[test]
    fn every_shipped_noise_settings_adds_the_beardifier_at_its_root() {
        let functions: BTreeMap<ResourceLocation, DensityFunctionHolder> =
            crate::corpus::registry("density_function");
        let settings: BTreeMap<ResourceLocation, crate::router::NoiseGeneratorSettings> =
            crate::corpus::registry("noise_settings");
        let placements: Vec<(String, BeardifierPlacement)> = settings
            .iter()
            .map(|(id, settings)| {
                let placement =
                    beardifier_placement(&settings.noise_router.final_density, &functions);
                (id.path().to_string(), placement)
            })
            .collect();
        let expected: Vec<(String, BeardifierPlacement)> = [
            "amplified",
            "beta",
            "caves",
            "end",
            "floating_islands",
            "large_biomes",
            "nether",
            "overworld",
        ]
        .into_iter()
        .map(|id| {
            let placement = if id == "beta" {
                BeardifierPlacement::Absent
            } else {
                BeardifierPlacement::RootAddend
            };
            (id.to_string(), placement)
        })
        .collect();
        assert_eq!(placements, expected);
    }

    #[test]
    fn a_beard_fills_below_its_ground_and_carves_above_it() {
        let beard = Beard::collect(
            CHUNK,
            vec![(
                TerrainAdaptation::BeardThin,
                jigsaw(cube([0, 63, 0], [8, 70, 8]), Projection::Rigid, Vec::new()),
            )],
        );
        assert!(beard.sample(4, 60, 4) > 0.0);
        assert!(beard.sample(4, 68, 4) < 0.0);
    }
}
