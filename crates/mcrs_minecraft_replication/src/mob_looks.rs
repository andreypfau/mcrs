use mcrs_minecraft_protocol_game::entity::MetadataEntry;

/// The entity data of a mob that its type does not fix. A field that holds the default of its
/// entry sends no entry.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MobLooks {
    pub no_ai: bool,
    pub left_handed: bool,
    /// The entries of the type that differ from their defaults, such as its variants, in the
    /// order of their indices. Each index is above the mob flags; the type table gives them
    /// ([`EntityTypeData::variant_indices`](mcrs_minecraft_entity::EntityTypeData)).
    pub type_entries: Vec<MetadataEntry<'static>>,
}
