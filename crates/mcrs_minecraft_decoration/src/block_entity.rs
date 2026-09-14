use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_worldgen::feature::tree::is_default;
use mcrs_minecraft_worldgen::value_provider::Weighted;
use mcrs_voxel_math::BlockPos;
use serde::{Deserialize, Serialize};

/// A block entity a generator produced, in the compound the save, the chunk
/// packet and the anvil reader all encode.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "id")]
pub enum GeneratedBlockEntity {
    #[serde(rename = "minecraft:beehive")]
    Beehive {
        x: i32,
        y: i32,
        z: i32,
        bees: Vec<BeeOccupant>,
    },
    /// The loot is not rolled here: the reference stores the table's id and its
    /// seed and rolls when a player first opens the chest.
    #[serde(rename = "minecraft:chest")]
    Chest {
        x: i32,
        y: i32,
        z: i32,
        #[serde(rename = "LootTable")]
        loot_table: String,
        #[serde(rename = "LootTableSeed", default, skip_serializing_if = "is_default")]
        loot_table_seed: i64,
    },
    #[serde(rename = "minecraft:mob_spawner")]
    MobSpawner {
        x: i32,
        y: i32,
        z: i32,
        #[serde(rename = "Delay")]
        delay: i16,
        #[serde(rename = "MinSpawnDelay")]
        min_spawn_delay: i16,
        #[serde(rename = "MaxSpawnDelay")]
        max_spawn_delay: i16,
        #[serde(rename = "SpawnCount")]
        spawn_count: i16,
        #[serde(rename = "MaxNearbyEntities")]
        max_nearby_entities: i16,
        #[serde(rename = "RequiredPlayerRange")]
        required_player_range: i16,
        #[serde(rename = "SpawnRange")]
        spawn_range: i16,
        #[serde(rename = "SpawnData")]
        spawn_data: SpawnData,
        #[serde(rename = "SpawnPotentials")]
        spawn_potentials: Vec<Weighted<SpawnData>>,
    },
    #[serde(rename = "minecraft:end_gateway")]
    EndGateway(EndGatewayData),
}

/// NBT has no boolean, so a flag is a byte. The tagged enum this sits inside
/// buffers the compound before it knows the variant, and a buffered byte never
/// reaches `deserialize_bool` — without this the whole gateway fails to load.
fn nbt_flag<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<bool, D::Error> {
    struct Flag;
    impl serde::de::Visitor<'_> for Flag {
        type Value = bool;

        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.write_str("a boolean or the byte standing for one")
        }

        fn visit_bool<E>(self, value: bool) -> Result<bool, E> {
            Ok(value)
        }

        fn visit_i64<E>(self, value: i64) -> Result<bool, E> {
            Ok(value != 0)
        }

        fn visit_u64<E>(self, value: u64) -> Result<bool, E> {
            Ok(value != 0)
        }
    }
    deserializer.deserialize_any(Flag)
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EndGatewayData {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    #[serde(rename = "Age")]
    pub age: i64,
    #[serde(
        rename = "exit_portal",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub exit_portal: Option<[i32; 3]>,
    #[serde(
        rename = "ExactTeleport",
        default,
        skip_serializing_if = "std::ops::Not::not",
        deserialize_with = "nbt_flag"
    )]
    pub exact_teleport: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SpawnData {
    pub entity: NbtCompound,
}

impl GeneratedBlockEntity {
    /// The `id` each variant is tagged with; a save entry naming any other kind
    /// is one this type does not describe.
    pub const IDS: [&'static str; 4] = [
        "minecraft:beehive",
        "minecraft:chest",
        "minecraft:mob_spawner",
        "minecraft:end_gateway",
    ];

    /// Which block the entity belongs to, which is what routes it to a column.
    pub fn position(&self) -> BlockPos {
        match self {
            GeneratedBlockEntity::Beehive { x, y, z, .. }
            | GeneratedBlockEntity::Chest { x, y, z, .. }
            | GeneratedBlockEntity::MobSpawner { x, y, z, .. } => BlockPos::new(*x, *y, *z),
            GeneratedBlockEntity::EndGateway(gateway) => {
                BlockPos::new(gateway.x, gateway.y, gateway.z)
            }
        }
    }

    pub fn chest(pos: BlockPos, loot_table: String, loot_table_seed: i64) -> Self {
        GeneratedBlockEntity::Chest {
            x: pos.x,
            y: pos.y,
            z: pos.z,
            loot_table,
            loot_table_seed,
        }
    }

    /// A spawner as `MonsterRoomFeature` leaves it: every timing field still at
    /// `BaseSpawner`'s constructed default, one entity in `SpawnData`, and the
    /// weighted list empty because nothing ever wrote it.
    pub fn mob_spawner(pos: BlockPos, entity_id: &str) -> Self {
        let mut entity = NbtCompound::new();
        entity.put_string("id", entity_id.to_owned());
        GeneratedBlockEntity::MobSpawner {
            x: pos.x,
            y: pos.y,
            z: pos.z,
            delay: 20,
            min_spawn_delay: 200,
            max_spawn_delay: 800,
            spawn_count: 4,
            max_nearby_entities: 6,
            required_player_range: 16,
            spawn_range: 4,
            spawn_data: SpawnData { entity },
            spawn_potentials: Vec::new(),
        }
    }
}

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
        entity_data.put_string("id", "minecraft:bee".to_owned());
        BeeOccupant {
            entity_data,
            ticks_in_hive,
            min_ticks_in_hive: BEE_MIN_TICKS_IN_HIVE,
        }
    }
}

/// The occupants a hive a generator wrote start with.
pub const BEE_MIN_TICKS_IN_HIVE: i32 = 600;
