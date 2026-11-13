use mcrs_minecraft_protocol_game::ProtoStack;
use mcrs_minecraft_protocol_game::entity::MobLooks;
use mcrs_minecraft_registries::Wire;

/// What the spawn bundle of an entity shows besides its motion.
#[derive(Clone, Debug, PartialEq)]
pub enum SpawnLook {
    Mob(MobLooks),
    /// The stack of an item entity (IEN-NET-001).
    Item(ProtoStack<Wire>),
}
