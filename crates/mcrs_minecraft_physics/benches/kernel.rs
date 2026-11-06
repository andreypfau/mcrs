//! The kernel targets of the physics design note, on one core with a warm cache.
#![expect(clippy::indexing_slicing, reason = "a dense test world indexes its sections")]

use std::hint::black_box;

use criterion::Criterion;
use criterion::criterion_group;
use criterion::criterion_main;
use glam::DVec3;
use mcrs_minecraft_physics::Aabb;
use mcrs_minecraft_physics::BirthKey;
use mcrs_minecraft_physics::CollisionSection;
use mcrs_minecraft_physics::CollisionTable;
use mcrs_minecraft_physics::CollisionWorld;
use mcrs_minecraft_physics::NoBodies;
use mcrs_minecraft_physics::Probe;
use mcrs_minecraft_physics::QueryEntity;
use mcrs_minecraft_physics::Scratch;
use mcrs_minecraft_physics::SectionLookup;
use mcrs_minecraft_physics::SectionMasks;
use mcrs_minecraft_physics::SourceKind;
use mcrs_minecraft_physics::Tables;
use mcrs_minecraft_physics::TestBlocks;
use mcrs_minecraft_physics::View;
use mcrs_minecraft_physics::WorldBorder;
use mcrs_minecraft_physics::resolve;
use mcrs_voxel::BlockStateId;
use mcrs_voxel::LocalPos;
use mcrs_voxel::SectionPos;

/// Sections in a flat array, as the engine stores them: 4 × 4 × 4 sections from the origin.
struct DenseWorld {
    sections: Vec<DenseSection>,
}

struct DenseSection {
    states: Box<[BlockStateId; 4096]>,
    masks: SectionMasks,
}

const SIDE: i32 = 4;

impl DenseWorld {
    fn new(table: &CollisionTable, fill: impl Fn(i32, i32, i32) -> BlockStateId) -> Self {
        let mut sections = Vec::new();
        for sy in 0..SIDE {
            for sz in 0..SIDE {
                for sx in 0..SIDE {
                    let mut states = Box::new([BlockStateId::AIR; 4096]);
                    for local in LocalPos::all() {
                        states[local.index()] = fill(
                            sx * 16 + i32::from(local.x()),
                            sy * 16 + i32::from(local.y()),
                            sz * 16 + i32::from(local.z()),
                        );
                    }
                    let masks = SectionMasks::from_states(table, 0, |local| states[local.index()]);
                    sections.push(DenseSection { states, masks });
                }
            }
        }
        Self { sections }
    }
}

impl CollisionSection for &DenseSection {
    fn masks(&self) -> Option<&SectionMasks> {
        Some(&self.masks)
    }

    fn state(&self, at: LocalPos) -> BlockStateId {
        self.states[at.index()]
    }
}

impl CollisionWorld for DenseWorld {
    type Section<'a> = &'a DenseSection;

    fn section(&self, at: SectionPos) -> SectionLookup<&DenseSection> {
        if !(0..SIDE).contains(&at.x) || !(0..SIDE).contains(&at.y) || !(0..SIDE).contains(&at.z) {
            return SectionLookup::Outside;
        }
        let index = ((at.y * SIDE + at.z) * SIDE + at.x) as usize;
        SectionLookup::Loaded(&self.sections[index])
    }

    fn border(&self) -> Option<&WorldBorder> {
        None
    }
}

fn probe(position: DVec3, displacement: DVec3, on_ground: bool) -> Probe {
    Probe {
        entity: QueryEntity {
            key: BirthKey::new(0, SourceKind::Command, 0, 0),
            position,
            fall_distance: 0.0,
            shift: false,
            powder_snow_solid: false,
            walks_on_powder_snow: false,
        },
        bounds: Aabb::of_body(position, 0.6, 1.95),
        displacement,
        step_height: 0.6,
        on_ground,
    }
}

fn bench_resolve(c: &mut Criterion) {
    let blocks = TestBlocks::new();
    let tables = Tables::new(blocks.table.clone());
    let floor = 20;
    let world = DenseWorld::new(&tables.collision, |x, y, _| {
        if y == floor {
            blocks.stone
        } else if y == floor + 1 && x == 33 {
            blocks.bottom_slab
        } else {
            BlockStateId::AIR
        }
    });
    let view = View {
        world: &world,
        bodies: &NoBodies,
        tables: &tables,
    };
    let mut scratch = Scratch::new();
    let top = f64::from(floor + 1);
    let cases = [
        (
            "resolve/inert_air",
            probe(
                DVec3::new(32.5, 40.0, 32.5),
                DVec3::new(0.0, -0.0784, 0.0),
                false,
            ),
        ),
        (
            "resolve/flat_ground",
            probe(
                DVec3::new(20.5, top, 20.5),
                DVec3::new(0.1, -0.0784, 0.05),
                true,
            ),
        ),
        (
            "resolve/step_up_slab",
            probe(
                DVec3::new(32.5, top, 32.5),
                DVec3::new(0.3, -0.0784, 0.0),
                true,
            ),
        ),
    ];
    let stepped = resolve(view, &cases[2].1, &mut scratch);
    assert!(stepped.step.is_some(), "the slab case steps up");
    for (name, probe) in cases {
        c.bench_function(name, |b| {
            b.iter(|| resolve(view, black_box(&probe), &mut scratch))
        });
    }
}

criterion_group!(kernel, bench_resolve);
criterion_main!(kernel);
