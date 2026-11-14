use crate::attribute_state::AttributeState;
use crate::mob_signal::MobSignal;
use crate::mob_spawn::MobSpawn;
use crate::mob_state::MobState;
use mcrs_minecraft_protocol_game::ProtoStack;
use mcrs_minecraft_protocol_game::entity::MetadataEntry;
use mcrs_minecraft_registries::Wire;

/// What a world publishes about a mob or an item entity in a tile delta. A spawn carries
/// [`MobReport::Spawn`], a move [`MobReport::Move`], and a data event [`MobReport::Signal`],
/// [`MobReport::Stack`], [`MobReport::Entries`] or [`MobReport::Attribute`].
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum MobReport {
    Spawn(MobSpawn),
    /// The state of a mob at the end of a tick in which it changed.
    Move(MobState),
    Signal(MobSignal),
    /// The new stack of an item entity (IEN-NET-003).
    Stack(ProtoStack<Wire>),
    /// The entries of an entity of [`SpawnLook::Custom`](crate::SpawnLook::Custom) that replace
    /// those of the same index. The next tracker tick sends the entries that changed
    /// (TRK-DATA-005). An entity of another look ignores it.
    Entries(Vec<MetadataEntry<'static>>),
    /// The instance of a syncable attribute of a mob after a change (ATR-NET-003).
    Attribute(AttributeState),
}
