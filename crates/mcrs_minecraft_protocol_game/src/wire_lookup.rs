use mcrs_minecraft_block::definition::BlockDefinitions;
use mcrs_minecraft_registries::{BlockStates, Namer, Registries, ResolveError, Resolver};
use mcrs_minecraft_resource::{
    Id, Registered, ResourceKeyBuf, ResourceLocationBuf, TagId, TagKeyBuf,
};

/// What a wire value is checked against. Both ends of a connection number a registry alike: a
/// client holds the registries its server sent, in the order sent.
#[derive(Clone, Copy)]
pub struct WireLookup<'a> {
    registries: &'a Registries,
    blocks: Option<&'a BlockDefinitions>,
}

impl<'a> WireLookup<'a> {
    pub fn new(registries: &'a Registries) -> Self {
        WireLookup {
            registries,
            blocks: None,
        }
    }

    /// Block states cross the wire by state id, which only the block definitions number.
    pub fn with_blocks(mut self, blocks: &'a BlockDefinitions) -> Self {
        self.blocks = Some(blocks);
        self
    }
}

impl Resolver for WireLookup<'_> {
    fn id<R: Registered>(&self, key: &ResourceKeyBuf<R>) -> Result<Id<R>, ResolveError> {
        self.registries.id(key)
    }

    fn tag<R: Registered>(&self, tag: &TagKeyBuf<R>) -> Result<TagId<R>, ResolveError> {
        self.registries.tag(tag)
    }
}

impl Namer for WireLookup<'_> {
    fn name<R: Registered>(&self, id: Id<R>) -> Option<&ResourceLocationBuf> {
        Namer::name(self.registries, id)
    }

    fn tag_name<R: Registered>(&self, tag: TagId<R>) -> Option<&ResourceLocationBuf> {
        self.registries.tag_name(tag)
    }
}

impl WireLookup<'_> {
    fn blocks(&self) -> Result<&BlockDefinitions, ResolveError> {
        self.blocks
            .ok_or_else(|| ResolveError::new("a block state needs the block definitions"))
    }
}

impl BlockStates for WireLookup<'_> {
    fn block_state(
        &self,
        id: u16,
    ) -> Result<(ResourceLocationBuf, Vec<(String, String)>), ResolveError> {
        self.blocks()?
            .properties_of(id)
            .ok_or_else(|| ResolveError::new(format!("no block state has id {id}")))
    }

    fn block_state_id(
        &self,
        block: &ResourceLocationBuf,
        properties: &[(&str, &str)],
    ) -> Result<u16, ResolveError> {
        self.blocks()?
            .state_id_of(block, properties)
            .ok_or_else(|| ResolveError::new(format!("{block} has no block state {properties:?}")))
    }
}
