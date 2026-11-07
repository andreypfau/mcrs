use crate::atlas::SpriteRegistry;
use crate::bake::BlockModels;
use crate::block_build::build_one;
use crate::property_table::PropertyTable;
use crate::resource_set::ResourceSet;
use bevy::math::Vec3;
use mcrs_minecraft_block::block_keys::Block;
use mcrs_minecraft_block::definition::BlockDefinitions;
use mcrs_minecraft_block::definition::schema::PropertyValue;
use mcrs_minecraft_core::Direction;
use mcrs_minecraft_mesh::block::BlockInfo;
use mcrs_minecraft_mesh::pack::{MAX_SPRITE_ARRAYS, MAX_SPRITES};
use mcrs_voxel::BlockStateId;

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
pub fn state_key(definitions: &BlockDefinitions, state: BlockStateId) -> Option<BlockStateKey> {
    let block = definitions.owner(state)?;
    let props = block
        .properties
        .0
        .iter()
        .filter_map(|property| {
            let value = block.value_of(state, &property.name)?;
            Some((property.name.to_string(), render(value)))
        })
        .collect();
    Some(BlockStateKey {
        name: block.identifier.as_str().to_owned(),
        props,
    })
}

fn render(value: &PropertyValue) -> String {
    match value {
        PropertyValue::Str(text) => text.to_string(),
        PropertyValue::Int(number) => number.to_string(),
        PropertyValue::Bool(flag) => flag.to_string(),
    }
}

pub struct Catalog {
    pub blocks: Vec<BlockInfo>,
    pub sprites: SpriteRegistry,
    pub failures: Vec<String>,
}

/// The four corners of a full cube's `dir` face, in the order a baked quad winds them.
pub fn cube_corners(dir: Direction) -> [Vec3; 4] {
    mcrs_minecraft_mesh::smooth_light::corners(dir, Vec3::ZERO, Vec3::ONE)
}

pub fn empty() -> Catalog {
    Catalog {
        blocks: Vec::new(),
        sprites: SpriteRegistry::default(),
        failures: Vec::new(),
    }
}

/// Bakes the states named by `ids`, which are indices into the corpus'
/// state space and so index the catalog directly.
pub fn extend(
    pack: &ResourceSet,
    catalog: &mut Catalog,
    definitions: &BlockDefinitions,
    ids: &[u16],
) {
    let mut models = BlockModels::new(pack);
    if catalog.blocks.len() < definitions.state_count() {
        catalog
            .blocks
            .resize(definitions.state_count(), BlockInfo::default());
    }
    for &id in ids {
        let Some(id_state) = BlockStateId::checked(id, definitions.state_count()) else {
            catalog.failures.push(format!(
                "state {id}: past the {} corpus states",
                definitions.state_count()
            ));
            continue;
        };
        let (Some(state), Some(block), Some(owner)) = (
            state_key(definitions, id_state),
            definitions.block_index(id_state).and_then(Block::from_id),
            definitions.owner(id_state),
        ) else {
            catalog
                .failures
                .push(format!("state {id}: no registry block owns it"));
            continue;
        };
        let data = definitions.state(id_state);
        match build_one(
            &mut models,
            block,
            &state,
            &PropertyTable::of_block(&owner.properties),
            data,
            definitions.shape(data.occlusion_shape),
            &mut catalog.sprites,
        ) {
            Ok(info) => {
                if let Some(slot) = catalog.blocks.get_mut(usize::from(id)) {
                    *slot = info;
                }
            }
            Err(reason) => catalog
                .failures
                .push(format!("{}: {reason}", state.label())),
        }
    }
    catalog.failures.extend(
        models
            .take_errors()
            .into_iter()
            .map(|error| format!("content error: {error}")),
    );
    assert!(
        catalog.sprites.arrays().len() <= MAX_SPRITE_ARRAYS,
        "the pack fills {} texture arrays, but the shaders bind only {MAX_SPRITE_ARRAYS}",
        catalog.sprites.arrays().len(),
    );
    assert!(
        catalog.sprites.len() <= MAX_SPRITES,
        "the pack has {} sprites, but a face can name only {MAX_SPRITES}",
        catalog.sprites.len(),
    );
}

#[cfg(test)]
pub fn corpus() -> &'static BlockDefinitions {
    &corpus_blocks().0
}

#[cfg(test)]
pub fn corpus_blocks() -> &'static mcrs_minecraft_block::definition::Blocks {
    &mcrs_minecraft_engine_registries::test_corpus().0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_global_state_id_names_the_block_and_every_property_it_stands_for() {
        let corpus = corpus();
        assert_eq!(
            state_key(corpus, corpus.default_state("minecraft:stone").unwrap())
                .unwrap()
                .label(),
            "minecraft:stone",
            "a block with no properties is named on its own"
        );

        let slab = corpus
            .block("minecraft:oak_slab")
            .expect("the corpus has oak slabs");
        for id in (0..slab.state_count).filter_map(|offset| slab.state_at(offset)) {
            let key = state_key(corpus, id).unwrap();
            assert_eq!(key.name, "minecraft:oak_slab");
            assert_eq!(key.props.len(), slab.properties.0.len());
            let walked = key
                .props
                .iter()
                .try_fold(slab.base_state_id, |at, (property, value)| {
                    slab.with_text(at, property, value)
                });
            assert_eq!(
                walked,
                Some(id),
                "{} does not lead back to the state it was read from",
                key.label(),
            );
        }
    }
}
