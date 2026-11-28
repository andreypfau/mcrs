use crate::attribute_state::AttributeState;
use crate::mob_signal::MobSignal;
use crate::mob_spawn::MobSpawn;
use crate::mob_state::MobState;
use mcrs_minecraft_protocol_game::ProtoStack;
use mcrs_minecraft_protocol_game::entity::MetadataEntry;
use mcrs_minecraft_registries::Wire;

#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum MobReport {
    Spawn(MobSpawn),
    Move(MobState),
    Signal(MobSignal),
    Stack(ProtoStack<Wire>),
    Entries(Vec<MetadataEntry<'static>>),
    Attribute(AttributeState),
}
