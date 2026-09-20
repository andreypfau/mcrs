use mcrs_minecraft_core::ResourceLocation;

pub trait RegistryLookup: Sync {
    fn id(&self, registry: &str, name: &ResourceLocation) -> Option<u32>;
    fn name(&self, registry: &str, id: u32) -> Option<&ResourceLocation>;

    /// The state id of `block` with `properties`, each unspecified property
    /// taking the block's default.
    fn block_state_id(&self, block: &ResourceLocation, properties: &[(&str, &str)]) -> Option<u32> {
        let _ = (block, properties);
        None
    }

    /// The block and every property of state `id`.
    fn block_state(&self, id: u32) -> Option<(ResourceLocation, Vec<(String, String)>)> {
        let _ = id;
        None
    }
}

pub struct ChainLookup<'a>(pub &'a [&'a dyn RegistryLookup]);

impl RegistryLookup for ChainLookup<'_> {
    fn id(&self, registry: &str, name: &ResourceLocation) -> Option<u32> {
        self.0.iter().find_map(|l| l.id(registry, name))
    }

    fn name(&self, registry: &str, id: u32) -> Option<&ResourceLocation> {
        self.0.iter().find_map(|l| l.name(registry, id))
    }

    fn block_state_id(&self, block: &ResourceLocation, properties: &[(&str, &str)]) -> Option<u32> {
        self.0
            .iter()
            .find_map(|l| l.block_state_id(block, properties))
    }

    fn block_state(&self, id: u32) -> Option<(ResourceLocation, Vec<(String, String)>)> {
        self.0.iter().find_map(|l| l.block_state(id))
    }
}

pub struct NoRegistries;

impl RegistryLookup for NoRegistries {
    fn id(&self, _: &str, _: &ResourceLocation) -> Option<u32> {
        None
    }

    fn name(&self, _: &str, _: u32) -> Option<&ResourceLocation> {
        None
    }
}
