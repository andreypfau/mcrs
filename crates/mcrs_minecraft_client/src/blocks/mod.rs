mod build;
mod tint;

use bevy::math::Vec3;

use mcrs_minecraft_protocol::BlockStateId;
use mcrs_minecraft_world::block::definition::BlockDefinitions;
use mcrs_minecraft_world::block::definition::schema::PropertyValue;

use crate::atlas::{Opacity, SpriteRef, SpriteRegistry};
use crate::bake::{Dir, TinyWorld};
use crate::model::Pack;
use crate::pack::{MAX_SPRITE_ARRAYS, MAX_SPRITES};

pub use build::Fluid;
pub use tint::tint_column;

use build::build_one;
use tint::extend_tints;

/// A block state as the resource pack names it: the block's identifier and
/// every property it declares, rendered the way a blockstates file spells them.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct BlockStateKey {
    pub name: String,
    pub props: Vec<(String, String)>,
}

impl BlockStateKey {
    pub fn pairs(&self) -> Vec<(&str, &str)> {
        self.props
            .iter()
            .map(|(key, value)| (key.as_str(), value.as_str()))
            .collect()
    }

    pub fn label(&self) -> String {
        if self.props.is_empty() {
            return self.name.clone();
        }
        let props: Vec<String> = self
            .props
            .iter()
            .map(|(key, value)| format!("{key}={value}"))
            .collect();
        format!("{}[{}]", self.name, props.join(","))
    }
}

/// The state the server's global block state id stands for.
pub fn state_key(definitions: &BlockDefinitions, id: u16) -> BlockStateKey {
    let state = BlockStateId(id);
    let block = definitions.owner(state);
    let props = block
        .properties
        .0
        .iter()
        .filter_map(|property| {
            let value = block.value_of(state, &property.name)?;
            Some((property.name.to_string(), render(value)))
        })
        .collect();
    BlockStateKey {
        name: block.identifier.as_str().to_owned(),
        props,
    }
}

fn render(value: &PropertyValue) -> String {
    match value {
        PropertyValue::Str(text) => text.to_string(),
        PropertyValue::Int(number) => number.to_string(),
        PropertyValue::Bool(flag) => flag.to_string(),
    }
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Pass {
    Solid = 0,
    Cutout = 1,
    Translucent = 2,
}

impl Pass {
    pub const COUNT: usize = 3;

    pub const fn from_index(index: usize) -> Pass {
        match index {
            0 => Pass::Solid,
            1 => Pass::Cutout,
            _ => Pass::Translucent,
        }
    }

    fn of(opacity: Opacity) -> Pass {
        match opacity {
            Opacity::Solid => Pass::Solid,
            Opacity::Cutout => Pass::Cutout,
            Opacity::Translucent => Pass::Translucent,
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum TintKind {
    Grass = 0,
    Foliage = 1,
    Water = 2,
}

pub const TINT_KINDS: usize = 3;

#[derive(Copy, Clone, Default)]
pub struct CubeFace {
    pub sprite: SpriteRef,
    pub pass: u8,
    pub tinted: bool,
}

#[derive(Clone)]
pub struct ModelQuad {
    pub positions: [Vec3; 4],
    pub uvs: [[f32; 2]; 4],
    pub cull: Option<Dir>,
    pub face: Option<u8>,
    pub sprite: SpriteRef,
    pub pass: Pass,
    pub shade: [u8; 4],
    pub tinted: bool,
}

#[derive(Clone)]
pub struct BlockInfo {
    pub cube: Option<[CubeFace; 6]>,
    pub quads: Vec<ModelQuad>,
    pub occludes: bool,
    pub self_culls: bool,
    pub sturdy: u8,
    pub tint_kind: TintKind,
    pub emission: u8,
    pub fluid: Option<Fluid>,
}

impl Default for BlockInfo {
    fn default() -> Self {
        Self {
            cube: None,
            quads: Vec::new(),
            occludes: false,
            self_culls: false,
            sturdy: 0,
            tint_kind: TintKind::Grass,
            emission: 0,
            fluid: None,
        }
    }
}

pub struct Catalog {
    pub blocks: Vec<BlockInfo>,
    pub sprites: SpriteRegistry,
    pub tints: Vec<[f32; 4]>,
    pub failures: Vec<String>,
}

pub const FACE_AXES: [[u8; 6]; 6] = [
    [1, 0, 0, 1, 2, 0],
    [1, 1, 0, 1, 2, 1],
    [2, 0, 0, 0, 1, 0],
    [2, 1, 0, 1, 1, 0],
    [0, 0, 2, 1, 1, 0],
    [0, 1, 2, 0, 1, 0],
];

pub const CORNER_UV: [[f32; 2]; 4] = [[0.0, 0.0], [0.0, 1.0], [1.0, 1.0], [1.0, 0.0]];

pub fn cube_corner(dir: Dir, corner: usize) -> Vec3 {
    crate::bake::corner(dir, corner, Vec3::ZERO, Vec3::ONE)
}
pub fn empty() -> Catalog {
    Catalog {
        blocks: Vec::new(),
        sprites: SpriteRegistry::new(),
        tints: vec![[1.0, 1.0, 1.0, 1.0]],
        failures: Vec::new(),
    }
}

/// Bakes the states named by `ids`, which are indices into the corpus'
/// state space and so index the catalog directly.
pub fn extend(
    pack: &Pack,
    catalog: &mut Catalog,
    definitions: &BlockDefinitions,
    ids: &[u16],
    biomes: &[String],
) {
    let neighbours = TinyWorld::default();
    if catalog.blocks.len() < definitions.state_count() {
        catalog
            .blocks
            .resize(definitions.state_count(), BlockInfo::default());
    }
    for &id in ids {
        let state = state_key(definitions, id);
        let data = definitions.state(BlockStateId(id));
        match build_one(pack, &state, data, &neighbours, &mut catalog.sprites) {
            Ok(info) => catalog.blocks[id as usize] = info,
            Err(reason) => catalog
                .failures
                .push(format!("{}: {reason}", state.label())),
        }
    }
    assert!(
        catalog.sprites.arrays().len() <= MAX_SPRITE_ARRAYS,
        "the pack uses {} sprite resolutions, but a packed quad can address only \
         {MAX_SPRITE_ARRAYS} arrays",
        catalog.sprites.arrays().len(),
    );
    for array in catalog.sprites.arrays() {
        assert!(
            array.stills() + catalog.sprites.animations().len() <= MAX_SPRITES,
            "{} still sprites are {}x{} and {} animations sit above them, but a quad can name \
             only {MAX_SPRITES} layers",
            array.stills(),
            array.size,
            array.size,
            catalog.sprites.animations().len(),
        );
    }

    extend_tints(pack, catalog, biomes);
}

/// The shipped block definitions, read through an asset server of their own:
/// a test has no running app to take them from.
#[cfg(test)]
pub fn corpus() -> &'static BlockDefinitions {
    use bevy::app::{App, TaskPoolPlugin};
    use bevy::asset::{AssetPlugin, AssetServer};
    use mcrs_minecraft_world::block::definition::load_block_definitions;

    static CORPUS: std::sync::OnceLock<BlockDefinitions> = std::sync::OnceLock::new();
    CORPUS.get_or_init(|| {
        let mut app = App::new();
        app.add_plugins(TaskPoolPlugin::default());
        app.add_plugins(AssetPlugin {
            watch_for_changes_override: Some(false),
            ..Default::default()
        });
        let assets = app.world().resource::<AssetServer>().clone();
        load_block_definitions(&assets)
            .expect("the block definition corpus loads")
            .0
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_global_state_id_names_the_block_and_every_property_it_stands_for() {
        let corpus = corpus();
        assert_eq!(
            state_key(corpus, corpus.default_state("minecraft:stone").0).label(),
            "minecraft:stone",
            "a block with no properties is named on its own"
        );

        let slab = corpus
            .block("minecraft:oak_slab")
            .expect("the corpus has oak slabs");
        for offset in 0..slab.state_count {
            let id = slab.base_state_id.0 + offset;
            let key = state_key(corpus, id);
            assert_eq!(key.name, "minecraft:oak_slab");
            assert_eq!(key.props.len(), slab.properties.0.len());
            let walked = key.props.iter().try_fold(slab.base_state_id, |at, (property, value)| {
                slab.with_text(at, property, value)
            });
            assert_eq!(
                walked,
                Some(BlockStateId(id)),
                "{} does not lead back to the state it was read from",
                key.label(),
            );
        }
    }
}
