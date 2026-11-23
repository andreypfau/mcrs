use bevy_app::App;
use bevy_app::FixedUpdate;
use bevy_ecs::entity::Entity;
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_ecs::schedule::LogLevel;
use bevy_ecs::schedule::ScheduleBuildSettings;
use bevy_ecs::system::SystemState;
use bevy_tasks::ComputeTaskPool;
use bevy_tasks::TaskPool;
use glam::DVec3;
use mcrs_engine_muxer::EntityId;
use mcrs_minecraft_damage::HitRecord;
use mcrs_minecraft_damage::Vitals;
use mcrs_minecraft_engine_entity_motion::MobIntent;
use mcrs_minecraft_engine_entity_motion::MotionId;
use mcrs_minecraft_engine_entity_motion::MotionKind;
use mcrs_minecraft_engine_entity_motion::MotionState;
use mcrs_minecraft_engine_world::birth_clock::BirthClock;
use mcrs_minecraft_engine_world::palette::ChunkBlocks;
use mcrs_minecraft_engine_world::palette::SectionStates;
use mcrs_minecraft_engine_world::tick::TickSet;
use mcrs_minecraft_engine_world::voxel_update::BlockPlaced;
use mcrs_minecraft_engine_world::world::dimension::Dimension;
use mcrs_minecraft_engine_world::world::dimension::DimensionTypeConfig;
use mcrs_minecraft_engine_world::world::storage::section::SectionIndex;
use mcrs_minecraft_engine_world::world::storage::view::WorldViewMut;
use mcrs_minecraft_entity_key::EntityUuid;
use mcrs_minecraft_entity_key::TileKey;
use mcrs_minecraft_physics::Body;
use mcrs_minecraft_physics::EntityState;
use mcrs_minecraft_physics::Living;
use mcrs_minecraft_physics::MotionRow;
use mcrs_minecraft_physics::MoveInput;
use mcrs_minecraft_physics::Steering;
use mcrs_minecraft_physics::Tables;
use mcrs_minecraft_physics::TestBlocks;
use mcrs_minecraft_physics::TestRng;
use mcrs_minecraft_simulation::BlockUpdateFlags;
use mcrs_minecraft_volume::BlockVolumeMut;
use mcrs_minecraft_volume::Denied;
use mcrs_minecraft_volume::SectionBlocksMut;
use mcrs_voxel::BlockPos;
use mcrs_voxel::BlockStateId;
use mcrs_voxel::LocalPos;
use mcrs_voxel::SectionPos;

use crate::EntityPhysicsPlugin;
use crate::PhysicsTables;
use crate::crowd_damage_type::CrowdDamageType;
use crate::floor::FLOOR;
use crate::mob_index::MobIndex;

const MIN_Y: i32 = -2048;
const HEIGHT: u32 = 4096;

pub struct TestScene {
    pub app: App,
    pub tick: u64,
    dimension: Entity,
}

impl TestScene {
    #[must_use]
    pub fn new(half: i32, walkers: u32, items: u32, seed: u64) -> Self {
        let test_blocks = TestBlocks::new();
        let mut scene = Self::empty(Tables::new(test_blocks.table));
        let edge = half * 32;
        let sections = (-edge / 16)..(edge / 16);
        for z in sections.clone() {
            for x in sections.clone() {
                scene.load_section(SectionPos::new(x, -1, z), test_blocks.stone);
                scene.load_section(SectionPos::new(x, 0, z), BlockStateId::AIR);
                scene.load_section(SectionPos::new(x, 1, z), BlockStateId::AIR);
            }
        }
        scene.lay_floor(sections, test_blocks.stone);
        let mut rng = TestRng(seed);
        let range = f64::from(edge) - 1.0;
        let mut spawns = Vec::new();
        for index in 0..walkers.saturating_add(items) {
            let key = EntityUuid::from_u128(u128::from(index));
            let x = rng.range(-range, range);
            let z = rng.range(-range, range);
            let (motion, body, living) = if index < walkers {
                let motion = MotionRow::zombie();
                let mut living = Living::of(&motion);
                living.orientation.yaw = rng.range(0.0, 360.0) as f32;
                if index % 2 == 0 {
                    living.steering = Some(Steering {
                        input: MoveInput {
                            forward: 1.0,
                            ..MoveInput::default()
                        },
                        jump: false,
                        speed: 0.1,
                    });
                }
                let body = Body {
                    position: DVec3::new(x, f64::from(FLOOR), z),
                    ..Body::default()
                };
                (motion, body, Some(living))
            } else {
                let body = Body {
                    position: DVec3::new(x, f64::from(FLOOR) + rng.range(0.0, 8.0), z),
                    velocity: DVec3::new(rng.range(-0.1, 0.1), 0.2, rng.range(-0.1, 0.1)),
                    ..Body::default()
                };
                (MotionRow::item(), body, None)
            };
            let mut state = EntityState::new(key, body, living);
            state.phase = (index % 4) as u8;
            spawns.push((EntityId(u64::from(index)), state, motion));
        }
        spawns.sort_by_key(|(_, state, _)| (TileKey::at(state.body.position), state.key));
        for (id, state, motion) in spawns {
            scene.spawn(id, state, motion);
        }
        scene
    }

    #[must_use]
    pub fn empty(tables: Tables) -> Self {
        ComputeTaskPool::get_or_init(TaskPool::default);
        let mut app = App::new();
        app.configure_sets(
            FixedUpdate,
            (
                TickSet::DomainInput,
                TickSet::ClientInput,
                TickSet::Simulate,
                TickSet::WorldWrite,
                TickSet::Project,
                TickSet::Output,
            )
                .chain(),
        )
        .insert_resource(PhysicsTables::new(tables))
        .add_plugins(EntityPhysicsPlugin);
        app.edit_schedule(FixedUpdate, |schedule| {
            schedule.set_build_settings(ScheduleBuildSettings {
                ambiguity_detection: LogLevel::Error,
                ..ScheduleBuildSettings::default()
            });
        });
        let dimension = app
            .world_mut()
            .spawn((
                Dimension,
                DimensionTypeConfig::new(MIN_Y, HEIGHT),
                SectionIndex::new(),
            ))
            .id();
        Self {
            app,
            tick: 0,
            dimension,
        }
    }

    pub fn load_section(&mut self, at: SectionPos, state: BlockStateId) {
        let world = self.app.world_mut();
        let section = world.spawn_empty().id();
        let Some(mut index) = world.get_mut::<SectionIndex>(self.dimension) else {
            return;
        };
        index.insert(at, section);
        let blocks = ChunkBlocks::new(SectionStates::homogeneous(state));
        index.land(at, section, blocks, None, false);
    }

    pub fn set_block(&mut self, at: BlockPos, state: BlockStateId) -> Result<BlockStateId, Denied> {
        let world = self.app.world_mut();
        let mut system: SystemState<WorldViewMut> = SystemState::new(world);
        let mut view = system.get_mut(world).map_err(|_| Denied::Unloaded)?;
        let mut blocks = view.get_mut(self.dimension).ok_or(Denied::Unloaded)?;
        blocks.set_block(at, state)
    }

    pub fn place_block(
        &mut self,
        at: BlockPos,
        state: BlockStateId,
    ) -> Result<BlockStateId, Denied> {
        let old_state = self.set_block(at, state)?;
        self.app.world_mut().write_message(BlockPlaced {
            chunk: Entity::PLACEHOLDER,
            chunk_pos: SectionPos::from(at),
            block_pos: at,
            old_state,
            new_state: state,
            flags: BlockUpdateFlags::empty(),
        });
        Ok(old_state)
    }

    pub fn set_crowd_damage_type(&mut self, damage_type: Option<u32>) {
        self.app
            .world_mut()
            .insert_resource(CrowdDamageType(damage_type));
    }

    pub fn spawn(&mut self, id: EntityId, state: EntityState, motion: MotionRow) -> Entity {
        self.spawn_with_vitals(id, state, motion, Vitals::default())
    }

    pub fn spawn_with_vitals(
        &mut self,
        id: EntityId,
        state: EntityState,
        motion: MotionRow,
        vitals: Vitals,
    ) -> Entity {
        let world = self.app.world_mut();
        let mut entity = world.spawn((MotionState(state), MotionKind(motion), MotionId(id)));
        if state.living.is_some() {
            entity.insert((MobIntent::default(), vitals, HitRecord::default()));
        }
        let entity = entity.id();
        world.resource_mut::<MobIndex>().insert(state.key, entity);
        entity
    }

    #[must_use]
    pub fn state(&self, entity: Entity) -> Option<EntityState> {
        self.app
            .world()
            .get::<MotionState>(entity)
            .map(|state| state.0)
    }

    pub fn states(&mut self) -> Vec<(Entity, EntityState)> {
        let world = self.app.world_mut();
        let mut query = world.query::<(Entity, &MotionState)>();
        let mut states: Vec<(Entity, EntityState)> = query
            .iter(world)
            .map(|(entity, state)| (entity, state.0))
            .collect();
        states.sort_by_key(|(_, state)| state.key);
        states
    }

    pub fn step(&mut self) {
        let world = self.app.world_mut();
        world.resource_mut::<BirthClock>().0 = self.tick;
        world.run_schedule(FixedUpdate);
        self.tick = self.tick.wrapping_add(1);
    }

    fn lay_floor(&mut self, sections: core::ops::Range<i32>, stone: BlockStateId) {
        let world = self.app.world_mut();
        let mut system: SystemState<WorldViewMut> = SystemState::new(world);
        let Ok(mut view) = system.get_mut(world) else {
            return;
        };
        let Some(mut blocks) = view.get_mut(self.dimension) else {
            return;
        };
        let top = u8::try_from(FLOOR - 1).unwrap_or_default();
        for z in sections.clone() {
            for x in sections.clone() {
                if let Ok(mut section) = blocks.section_mut(SectionPos::new(x, 0, z)) {
                    section.fill_box(LocalPos::new(0, 0, 0), LocalPos::new(15, top, 15), stone);
                }
            }
        }
    }
}
