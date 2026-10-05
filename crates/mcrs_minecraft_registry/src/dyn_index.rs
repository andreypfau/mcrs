use crate::NameTable;
use mcrs_minecraft_core::resource_location::ResourceLocation;
use mcrs_minecraft_core::tag_key::TaggedRegistry;
use std::marker::PhantomData;
use std::sync::Arc;

/// A dense `ResourceLocation`-to-`u32` index for dynamic registry types,
/// numbered by the registry loader's table.
#[cfg_attr(feature = "bevy", derive(bevy_ecs::resource::Resource))]
pub struct DynRegistryIndex<T: TaggedRegistry> {
    table: Arc<NameTable>,
    _marker: PhantomData<fn() -> T>,
}

impl<T: TaggedRegistry> DynRegistryIndex<T> {
    pub fn from_table(table: &Arc<NameTable>) -> Self {
        Self {
            table: Arc::clone(table),
            _marker: PhantomData,
        }
    }

    pub fn get(&self, rl: &str) -> Option<u32> {
        self.table.number(rl).map(u32::from)
    }

    pub fn location(&self, id: u32) -> Option<&ResourceLocation<Arc<str>>> {
        self.table.name(id as usize)
    }

    pub fn len(&self) -> u32 {
        self.table.len() as u32
    }

    pub fn is_empty(&self) -> bool {
        self.table.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestBiome;
    impl TaggedRegistry for TestBiome {
        const REGISTRY_PATH: &'static str = "worldgen/biome";
    }

    fn rl_arc(s: &str) -> ResourceLocation<Arc<str>> {
        ResourceLocation::parse(s).unwrap()
    }

    #[test]
    fn index_follows_the_table_order() {
        let table = Arc::new(
            NameTable::new(
                rl_arc("minecraft:worldgen/biome"),
                [
                    rl_arc("minecraft:plains"),
                    rl_arc("minecraft:desert"),
                    rl_arc("minecraft:forest"),
                ],
                [],
            )
            .unwrap(),
        );
        let index = DynRegistryIndex::<TestBiome>::from_table(&table);
        assert_eq!(index.len(), 3);
        assert_eq!(index.get("minecraft:plains"), Some(0));
        assert_eq!(index.get("minecraft:desert"), Some(1));
        assert_eq!(index.get("minecraft:forest"), Some(2));
        assert_eq!(index.location(1).unwrap().as_str(), "minecraft:desert");
    }
}
