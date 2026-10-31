use bevy_ecs::resource::Resource;
use mcrs_minecraft_dimension::Dimension;
use mcrs_minecraft_engine_world::world::dimension_list::DimensionList;
use mcrs_minecraft_resource::ResourceKeyBuf;
use mcrs_minecraft_world::worldgen::chunk_generator::ChunkGenerator;

/// The chunk generator of each dimension, in the order of the [`DimensionList`].
#[derive(Resource, Clone, Debug, Default)]
pub struct DimensionGenerators(pub Vec<ChunkGenerator>);

impl DimensionGenerators {
    pub fn iter<'a>(
        &'a self,
        dimensions: &'a DimensionList,
    ) -> impl Iterator<Item = (&'a ResourceKeyBuf<Dimension>, &'a ChunkGenerator)> {
        dimensions.keys().iter().zip(&self.0)
    }

    pub fn get(
        &self,
        dimensions: &DimensionList,
        dimension: &ResourceKeyBuf<Dimension>,
    ) -> Option<&ChunkGenerator> {
        let index = dimensions.keys().iter().position(|key| key == dimension)?;
        self.0.get(index)
    }
}
