use std::sync::LazyLock;

use mcrs_minecraft_biome::Biome;
use mcrs_minecraft_block::definition::{BlockDefinitions, FluidState};
use mcrs_minecraft_block::fluid_keys::Fluid;
use mcrs_minecraft_dimension::Dimension;
use mcrs_minecraft_engine_registries::test_corpus;
use mcrs_minecraft_entity::spawn_condition::Structure;
use mcrs_minecraft_game_registries::registry_load::test_registries;
use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_predicate::{LocationView, SmokeTable};
use mcrs_minecraft_registries::Registries;
use mcrs_minecraft_resource::{Id, ResourceLocationBuf};
use mcrs_voxel::{BlockPos, BlockStateId};

pub fn registries() -> &'static Registries {
    test_registries()
}

pub fn definitions() -> &'static BlockDefinitions {
    &test_corpus().0.0
}

static SMOKE: LazyLock<SmokeTable> =
    LazyLock::new(|| SmokeTable::bake(registries(), definitions()).expect("block tags"));

pub fn smoke() -> &'static SmokeTable {
    &SMOKE
}

pub fn id<R>(registry: &str, name: &str) -> Id<R> {
    let location = |text: &str| ResourceLocationBuf::read(text).expect("a resource location");
    let number = registries()
        .id_of(&location(registry), &location(name))
        .unwrap_or_else(|| panic!("{registry} has no {name}"));
    Id::checked(number, usize::MAX).expect("a registry id")
}

pub fn state(block: &str, properties: &[(&str, &str)]) -> BlockStateId {
    let entry = definitions().block(block).expect("a vanilla block");
    properties
        .iter()
        .fold(entry.default_state_id, |state, (name, value)| {
            entry
                .with_text(state, name, value)
                .unwrap_or_else(|| panic!("{block} has no {name}={value}"))
        })
}

#[derive(Default)]
pub struct Grid {
    pub blocks: Vec<(BlockPos, BlockStateId)>,
    pub stateless: Vec<BlockPos>,
    pub fluid: Option<FluidState>,
    pub block_light: Option<u8>,
    pub sky_light: Option<u8>,
    pub sky_light_level: Option<f32>,
    pub biome: Option<Id<Biome>>,
    pub structures: Vec<Id<Structure>>,
    pub dimension: Option<Id<Dimension>>,
    pub block_entity: Option<NbtCompound>,
}

impl LocationView for Grid {
    fn block_state(&self, pos: BlockPos) -> Option<BlockStateId> {
        self.blocks
            .iter()
            .find(|(at, _)| *at == pos)
            .map(|&(_, state)| state)
    }

    fn is_loaded(&self, pos: BlockPos) -> bool {
        self.block_state(pos).is_some() || self.stateless.contains(&pos)
    }

    fn fluid_state(&self, pos: BlockPos) -> Option<FluidState> {
        self.is_loaded(pos).then(|| {
            self.fluid.unwrap_or(FluidState {
                fluid: Fluid::Empty.id(),
                level: 0,
                source: false,
            })
        })
    }

    fn block_light(&self, pos: BlockPos) -> Option<u8> {
        self.block_light.filter(|_| self.is_loaded(pos))
    }

    fn sky_light(&self, pos: BlockPos) -> Option<u8> {
        self.sky_light.filter(|_| self.is_loaded(pos))
    }

    fn sky_light_level(&self) -> Option<f32> {
        self.sky_light_level
    }

    fn biome(&self, pos: BlockPos) -> Option<Id<Biome>> {
        self.biome.filter(|_| self.is_loaded(pos))
    }

    fn inside_structure(&self, pos: BlockPos, structure: Id<Structure>) -> Option<bool> {
        self.is_loaded(pos)
            .then(|| self.structures.contains(&structure))
    }

    fn dimension(&self) -> Option<Id<Dimension>> {
        self.dimension
    }

    fn block_entity_data(&self, pos: BlockPos, data: &mut NbtCompound) -> bool {
        match (&self.block_entity, self.is_loaded(pos)) {
            (Some(saved), true) => {
                data.merge(saved);
                true
            }
            _ => false,
        }
    }
}
