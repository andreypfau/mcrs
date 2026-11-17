use mcrs_minecraft_protocol_game::ProtoStack;
use mcrs_minecraft_protocol_game::block::BlockStateNumber;
use mcrs_minecraft_protocol_game::entity::{MetadataEntry, MobLooks};
use mcrs_minecraft_registries::Wire;
use mcrs_voxel::BlockPos;

#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum SpawnLook {
    Mob(MobLooks),
    Item(ProtoStack<Wire>),
    FallingBlock {
        state: BlockStateNumber,
        start: BlockPos,
    },
    Custom {
        data: i32,
        entries: Vec<MetadataEntry<'static>>,
    },
}
