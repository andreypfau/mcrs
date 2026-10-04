use std::collections::HashSet;
use std::sync::{Arc, OnceLock};

use bevy_app::{App, TaskPoolPlugin};
use bevy_asset::{AssetPlugin, AssetServer};
use mcrs_minecraft_assets::tag::file::SerializedTagFile;
use mcrs_minecraft_assets::tag::{DynTagRegistry, TagLoader};
use mcrs_minecraft_block::Block;
use mcrs_minecraft_block::definition::{BlockStateFlags, Blocks};
use mcrs_minecraft_block::light::block_light_registry;
use mcrs_minecraft_chunk::VoxelId;
use mcrs_minecraft_core::{ResourceLocation, TaggedRegistry};
use mcrs_minecraft_light::block::LightRegistry;
use mcrs_minecraft_light_color::asset::{BlockStateRef, StateTarget};
use mcrs_minecraft_light_color::colors::{LightColors, LightType};
use mcrs_minecraft_registry::BlockStateId;
use mcrs_minecraft_world::item::test_corpus;
use mcrs_minecraft_worldgen_testing::{assets_dir, json_files};

use crate::fixture::FixtureState;

/// The shipped block corpus with the server's light table and the loaded
/// colour table: the only source of a fixture's light rows.
pub struct Corpus {
    pub blocks: &'static Blocks,
    pub registry: Arc<LightRegistry>,
    pub colours: LightColors,
}

impl Corpus {
    pub fn get() -> &'static Corpus {
        static CORPUS: OnceLock<Corpus> = OnceLock::new();
        CORPUS.get_or_init(|| {
            let blocks = &test_corpus().0;
            let colours = LightColors::load(&asset_server(), blocks, &block_tags(blocks))
                .unwrap_or_else(|e| panic!("the light colour table does not load: {e}"));
            Corpus {
                blocks,
                registry: block_light_registry(blocks),
                colours,
            }
        })
    }

    /// Fixtures are committed data, so an entry the corpus cannot name is a
    /// hard failure rather than a fallback to some default state.
    pub fn resolve(&self, state: &BlockStateRef) -> VoxelId {
        let StateTarget::Block(id) = &state.target else {
            panic!("`{state}` names a tag, not a block state");
        };
        let block = self
            .blocks
            .block(id.as_str())
            .unwrap_or_else(|| panic!("`{state}` names a block the corpus lacks"));
        let mut resolved = block.default_state_id;
        for (name, value) in &state.properties {
            resolved = block
                .with_text(resolved, name, value)
                .unwrap_or_else(|| panic!("`{state}`: the block declares no `{name}={value}`"));
        }
        VoxelId(resolved.0)
    }

    pub fn name(&self, id: VoxelId) -> BlockStateRef {
        let state = BlockStateId(id.0);
        let block = self.blocks.owner(state);
        BlockStateRef {
            target: StateTarget::Block(block.identifier.clone()),
            properties: block
                .properties
                .0
                .iter()
                .map(|property| {
                    let value = block
                        .value_of(state, &property.name)
                        .expect("a state holds a value for every property of its block");
                    (property.name.to_string(), value.to_text())
                })
                .collect(),
        }
    }

    /// The whole colour table and, per state, the light row the server's
    /// table gives it.
    pub fn bake(&self, states: &[VoxelId]) -> (Vec<[u8; 3]>, Vec<FixtureState>) {
        let colours = (1..self.colours.type_count())
            .map(|t| {
                self.colours
                    .rgb(LightType(t as u8))
                    .expect("every type past the default has a colour")
            })
            .collect();
        let palette = states
            .iter()
            .map(|&id| {
                let data = self.blocks.state(BlockStateId(id.0));
                let occlusion = data
                    .flags
                    .contains(BlockStateFlags::USE_SHAPE_FOR_LIGHT_OCCLUSION)
                    .then(|| {
                        self.blocks
                            .shape(data.occlusion_shape)
                            .iter()
                            .map(|b| [b.min.to_array(), b.max.to_array()])
                            .collect()
                    });
                FixtureState {
                    state: self.name(id),
                    dampening: data.light_dampening,
                    emission: data.light_emission,
                    occlusion,
                    light_type: self.colours.light_type(id).0,
                }
            })
            .collect();
        (colours, palette)
    }
}

fn asset_server() -> AssetServer {
    let mut app = App::new();
    app.add_plugins((
        TaskPoolPlugin::default(),
        AssetPlugin {
            watch_for_changes_override: Some(false),
            ..Default::default()
        },
    ));
    app.world().resource::<AssetServer>().clone()
}

fn block_tags(blocks: &Blocks) -> DynTagRegistry<Block> {
    let mut loader = TagLoader::<Block, u32>::default();
    for namespace in std::fs::read_dir(assets_dir()).expect("the assets directory exists") {
        let namespace = namespace.expect("the assets directory lists").path();
        let dir = namespace.join("tags").join(Block::REGISTRY_PATH);
        if !dir.is_dir() {
            continue;
        }
        let namespace = namespace
            .file_name()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        for path in json_files(&dir) {
            let name = path.strip_prefix(&dir).unwrap().with_extension("");
            let name = format!("{namespace}:{}", name.to_string_lossy().replace('\\', "/"));
            let mut members = HashSet::new();
            collect(blocks, &name, &mut members);
            loader.insert(ResourceLocation::read(&name).unwrap(), members);
        }
    }
    loader.freeze(blocks)
}

fn collect(blocks: &Blocks, name: &str, into: &mut HashSet<u32>) {
    let location = ResourceLocation::read(name).unwrap();
    let path = assets_dir()
        .join(location.namespace())
        .join("tags")
        .join(Block::REGISTRY_PATH)
        .join(format!("{}.json", location.path()));
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let file: SerializedTagFile =
        serde_json::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    for entry in file.values {
        if entry.id.is_tag {
            collect(blocks, entry.id.loc.as_str(), into);
        } else if let Some(index) = blocks.index_of(entry.id.loc.as_str()) {
            into.insert(index);
        }
    }
}
