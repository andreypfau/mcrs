//! The kernel targets of the physics design note, on one core with a warm cache.
#![expect(
    clippy::indexing_slicing,
    reason = "a dense test world indexes its sections"
)]

use std::hint::black_box;

use criterion::Criterion;
use criterion::criterion_group;
use criterion::criterion_main;
use glam::DVec3;
use mcrs_minecraft_entity_key::BirthKey;
use mcrs_minecraft_entity_key::KeyedStream;
use mcrs_minecraft_entity_key::SourceKind;
use mcrs_minecraft_physics::Aabb;
use mcrs_minecraft_physics::Body;
use mcrs_minecraft_physics::CollisionSection;
use mcrs_minecraft_physics::CollisionTable;
use mcrs_minecraft_physics::CollisionWorld;
use mcrs_minecraft_physics::EntityState;
use mcrs_minecraft_physics::GridRow;
use mcrs_minecraft_physics::Keys;
use mcrs_minecraft_physics::ListedBodies;
use mcrs_minecraft_physics::Living;
use mcrs_minecraft_physics::MotionRow;
use mcrs_minecraft_physics::NoBodies;
use mcrs_minecraft_physics::PlayerInput;
use mcrs_minecraft_physics::PlayerRotation;
use mcrs_minecraft_physics::PlayerState;
use mcrs_minecraft_physics::Probe;
use mcrs_minecraft_physics::PushRules;
use mcrs_minecraft_physics::Scratch;
use mcrs_minecraft_physics::SectionLookup;
use mcrs_minecraft_physics::SectionMasks;
use mcrs_minecraft_physics::Side;
use mcrs_minecraft_physics::Tables;
use mcrs_minecraft_physics::TestBlocks;
use mcrs_minecraft_physics::TestFixture;
use mcrs_minecraft_physics::View;
use mcrs_minecraft_physics::WorldBorder;
use mcrs_minecraft_physics::push_entity;
use mcrs_minecraft_physics::resolve;
use mcrs_minecraft_physics::tick_entity;
use mcrs_minecraft_physics::tick_player;
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
                    let masks = SectionMasks::from_states(table, |local| states[local.index()]);
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
        entity: TestFixture::query_entity(position),
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
    let view = View::new(&world, &NoBodies, &tables);
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

fn bench_entities(c: &mut Criterion) {
    let blocks = TestBlocks::new();
    let tables = Tables::new(blocks.table.clone());
    let floor = 20;
    let world = DenseWorld::new(&tables.collision, |_, y, _| {
        if y == floor {
            blocks.stone
        } else {
            BlockStateId::AIR
        }
    });
    let view = View::new(&world, &NoBodies, &tables);
    let mut scratch = Scratch::new();
    let mut actions = Vec::new();
    let key = BirthKey::new(0, SourceKind::Command, 0, 0);
    let walker_row = MotionRow::zombie();
    let mut walker = EntityState::new(
        key,
        Body {
            position: DVec3::new(20.5, f64::from(floor + 1), 20.5),
            velocity: DVec3::new(0.1, -0.0784, 0.05),
            ..Body::default()
        },
        Some(Living::default()),
    );
    walker.body.flags.on_ground = true;
    c.bench_function("tick_entity/walker_on_ground", |b| {
        b.iter(|| {
            let mut state = walker;
            state.body.velocity.x = 0.1;
            actions.clear();
            tick_entity(
                view,
                SERVER,
                &walker_row,
                black_box(&mut state),
                true,
                &mut actions,
                &mut scratch,
            )
        })
    });
    let item_row = MotionRow::item();
    let item = EntityState::new(
        key,
        Body {
            position: DVec3::new(32.5, 50.0, 32.5),
            velocity: DVec3::new(0.01, 0.2, -0.01),
            ..Body::default()
        },
        None,
    );
    c.bench_function("tick_entity/item_in_air", |b| {
        b.iter(|| {
            let mut state = item;
            actions.clear();
            tick_entity(
                view,
                SERVER,
                &item_row,
                black_box(&mut state),
                false,
                &mut actions,
                &mut scratch,
            )
        })
    });
}

fn bench_player(c: &mut Criterion) {
    let blocks = TestBlocks::new();
    let tables = Tables::new(blocks.table.clone());
    let floor = 20;
    let world = DenseWorld::new(&tables.collision, |_, y, _| {
        if y == floor {
            blocks.stone
        } else {
            BlockStateId::AIR
        }
    });
    let view = View::new(&world, &NoBodies, &tables);
    let mut scratch = Scratch::new();
    let mut actions = Vec::new();
    let mut state = PlayerState::new(DVec3::new(30.5, f64::from(floor + 1), 30.5));
    let mut out = state;
    let input = PlayerInput {
        keys: Keys {
            forward: true,
            ..Keys::default()
        },
        rotation: PlayerRotation {
            yaw: 30.0,
            pitch: 0.0,
        },
        ..PlayerInput::default()
    };
    for _ in 0..3 {
        tick_player(
            view,
            Side::OwnClient,
            &state,
            &PlayerInput::default(),
            &mut out,
            &mut actions,
            &mut scratch,
        );
        state = out;
    }
    c.bench_function("tick_player/walk_on_ground", |b| {
        b.iter(|| {
            actions.clear();
            tick_player(
                view,
                Side::OwnClient,
                black_box(&state),
                &input,
                &mut out,
                &mut actions,
                &mut scratch,
            );
            out.velocity
        })
    });
}

fn bench_push(c: &mut Criterion) {
    let mut scratch = Scratch::new();
    for count in [3_u32, 24] {
        let rows: Vec<GridRow> = (0..=count)
            .map(|index| {
                TestFixture::mob(
                    index,
                    0.5 + f64::from(index % 5) * 0.1,
                    0.5 + f64::from(index / 5) * 0.1,
                )
            })
            .collect();
        let bodies = ListedBodies {
            colliders: Vec::new(),
            rows: rows.clone(),
        };
        let of = rows[0];
        let stream = KeyedStream::new(&of.key, 1);
        let rules = PushRules {
            max_entity_cramming: 64,
            cap: 24,
        };
        c.bench_function(&format!("push_gather/{count}_neighbours"), |b| {
            b.iter(|| {
                let mut velocity = DVec3::ZERO;
                push_entity(
                    &bodies,
                    black_box(&of),
                    &rules,
                    &stream,
                    &mut velocity,
                    &mut scratch,
                );
                velocity
            })
        });
    }
}

const SERVER: Side = Side::Server { authority: true };

criterion_group!(
    kernel,
    bench_resolve,
    bench_entities,
    bench_push,
    bench_player
);
criterion_main!(kernel);
