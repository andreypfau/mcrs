use mcrs_core::tag::key::TaggedRegistry;
use mcrs_core::ResourceLocation;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EntityType {
    pub identifier: ResourceLocation<&'static str>,
    pub protocol_id: u32,
}

impl EntityType {
    pub const fn new(identifier: ResourceLocation<&'static str>, protocol_id: u32) -> Self {
        Self {
            identifier,
            protocol_id,
        }
    }
}

impl TaggedRegistry for EntityType {
    const REGISTRY_PATH: &'static str = "entity_type";
}
