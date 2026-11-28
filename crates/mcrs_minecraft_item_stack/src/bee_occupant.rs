use crate::EntityData;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct BeeOccupant {
    pub entity_data: EntityData,
    pub ticks_in_hive: i32,
    pub min_ticks_in_hive: i32,
}
