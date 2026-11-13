use crate::mob_data::MobData;

/// How an entity type lives, which picks the rules of its tick.
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub enum Life {
    /// A living entity that is not a mob, such as a player.
    Living,
    /// A mob: its body follows its head.
    Mob(MobData),
    /// An entity that is not living, such as an item entity.
    Lifeless,
}
