use mcrs_minecraft_nbt::compound::NbtCompound;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BeeOccupant {
    pub entity_data: NbtCompound,
    pub ticks_in_hive: i32,
    pub min_ticks_in_hive: i32,
}

impl BeeOccupant {
    pub fn bee(ticks_in_hive: i32) -> Self {
        let mut entity_data = NbtCompound::new();
        entity_data.put_string(
            "id",
            mcrs_minecraft_entity::entity_type_keys::EntityType::Bee
                .as_static_str()
                .to_owned(),
        );
        BeeOccupant {
            entity_data,
            ticks_in_hive,
            min_ticks_in_hive: BEE_MIN_TICKS_IN_HIVE,
        }
    }
}

pub const BEE_MIN_TICKS_IN_HIVE: i32 = 600;
