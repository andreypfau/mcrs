use mcrs_minecraft_resource::ResourceLocation;

use crate::NameTable;

pub trait NameLookup {
    fn id(&self, table: NameTable, name: ResourceLocation<'_>) -> Option<u16>;

    fn name(&self, table: NameTable, id: u16) -> Option<ResourceLocation<'_>>;
}
