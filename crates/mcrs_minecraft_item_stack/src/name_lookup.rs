use mcrs_minecraft_resource::ResourceLocation;

use crate::NameTable;

/// The names of the entries that a codec reads and writes as identifiers.
///
/// The two methods are the two directions of one mapping: `id` of the result of `name` gives the
/// same numeric id back.
pub trait NameLookup {
    /// The numeric id of the entry with this name, or `None` when the table has no such entry.
    fn id(&self, table: NameTable, name: ResourceLocation<'_>) -> Option<u16>;

    /// The name of the entry with this numeric id, or `None` when the table has no such entry.
    fn name(&self, table: NameTable, id: u16) -> Option<ResourceLocation<'_>>;
}
