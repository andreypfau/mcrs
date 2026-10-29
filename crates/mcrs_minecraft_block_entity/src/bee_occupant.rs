use mcrs_minecraft_nbt::compound::NbtCompound;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BeeOccupant {
    /// `TypedEntityData`: the occupant's entity type under `id`, with whatever
    /// else it carries. Required by the reference's codec, which fails the whole
    /// `bees` list without it and loads the hive empty.
    pub entity_data: NbtCompound,
    pub ticks_in_hive: i32,
    pub min_ticks_in_hive: i32,
}

impl BeeOccupant {
    /// `Occupant.create`: a bee with an empty tag, which is what worldgen puts
    /// in a nest.
    pub fn bee(ticks_in_hive: i32) -> Self {
        let mut entity_data = NbtCompound::new();
        entity_data.put_string(
            "id",
            mcrs_minecraft_entity::keys::EntityType::Bee
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

/// The occupants a hive a generator wrote start with.
pub const BEE_MIN_TICKS_IN_HIVE: i32 = 600;
