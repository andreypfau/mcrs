use mcrs_minecraft_protocol_game::ProtoStack;
use mcrs_minecraft_protocol_game::block::BlockStateNumber;
use mcrs_minecraft_protocol_game::entity::{MetadataEntry, MobLooks};
use mcrs_minecraft_registries::Wire;
use mcrs_voxel::BlockPos;

/// What the spawn bundle of an entity shows besides its motion.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum SpawnLook {
    Mob(MobLooks),
    /// The stack of an item entity (IEN-NET-001).
    Item(ProtoStack<Wire>),
    /// The carried state and the start position of a falling block (FBK-SPN-002, FBK-DAT-003).
    FallingBlock {
        state: BlockStateNumber,
        start: BlockPos,
    },
    /// An entity of a kind that this crate does not know. Its spawn bundle has the `add_entity`
    /// with `data`, then the `set_entity_data` with `entries` when there is one; a later
    /// [`MobReport::Entries`](crate::MobReport::Entries) changes them. `entries` holds only the
    /// entries that differ from their defaults, with at most one entry for each index.
    Custom {
        data: i32,
        entries: Vec<MetadataEntry<'static>>,
    },
}
