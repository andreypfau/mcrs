use crate::mob_data::MobData;

#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub enum Life {
    Living,
    Mob(MobData),
    Lifeless,
}
