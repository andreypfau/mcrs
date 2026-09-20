use mcrs_minecraft_core::ResourceLocation;

pub trait RegistryLookup: Sync {
    fn id(&self, registry: &str, name: &ResourceLocation) -> Option<u32>;
    fn name(&self, registry: &str, id: u32) -> Option<&ResourceLocation>;
}

pub struct ChainLookup<'a>(pub &'a [&'a dyn RegistryLookup]);

impl RegistryLookup for ChainLookup<'_> {
    fn id(&self, registry: &str, name: &ResourceLocation) -> Option<u32> {
        self.0.iter().find_map(|l| l.id(registry, name))
    }

    fn name(&self, registry: &str, id: u32) -> Option<&ResourceLocation> {
        self.0.iter().find_map(|l| l.name(registry, id))
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
