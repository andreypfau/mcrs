use mcrs_minecraft_protocol_game::ProtoStack;
use mcrs_minecraft_protocol_game::block::BlockStateNumber;
use mcrs_minecraft_protocol_game::entity::MobLooks;
use mcrs_minecraft_registries::Wire;
use mcrs_voxel::BlockPos;

/// What the spawn bundle of an entity shows besides its motion.
#[derive(Clone, Debug, PartialEq)]
pub enum SpawnLook {
    Mob(MobLooks),
    /// The stack of an item entity (IEN-NET-001).
    Item(ProtoStack<Wire>),
    /// The carried state and the start position of a falling block (FBK-SPN-002, FBK-DAT-003).
    FallingBlock {
        state: BlockStateNumber,
        start: BlockPos,
    },
}
