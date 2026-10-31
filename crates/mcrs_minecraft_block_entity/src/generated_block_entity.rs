use std::io::Cursor;

use mcrs_minecraft_block::keys::Block;
use mcrs_minecraft_block::keys::BlockEntityType;
use mcrs_minecraft_core::codec::is_default;
use mcrs_minecraft_item::ItemStackWithSlot;
use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_nbt::tag::NbtTag;
use mcrs_minecraft_nbt::{Nbt, nbt_flag, nbt_int_array};
use mcrs_minecraft_registries::{Named, dispatch};
use mcrs_minecraft_resource::{ResourceLocation, rl};
use mcrs_minecraft_value_provider::Weighted;
use mcrs_voxel::BlockPos;

use crate::{
    BannerLayer, BeeOccupant, ContainerData, EMPTY_LABEL, EndGatewayData, FurnaceData, HopperData,
    Joint, SavedItem, SignText, SpawnData, Text, Uuid, VaultConfig,
};

const EMPTY_TEMPLATE_POOL: ResourceLocation<'static> = rl!("minecraft:empty");

fn empty_id() -> String {
    EMPTY_LABEL.to_string()
}

fn empty_pool() -> String {
    EMPTY_TEMPLATE_POOL.to_string()
}

fn air_id() -> String {
    Block::Air.as_static_str().to_owned()
}

/// A block entity a generator produced, in the compound the save, the chunk
/// packet and the anvil reader all encode.
#[dispatch(registry = mcrs_minecraft_block::keys::BlockEntityType, key = "id", only)]
#[derive(Clone, Debug, PartialEq)]
pub enum GeneratedBlockEntity {
    Beehive {
        x: i32,
        y: i32,
        z: i32,
        bees: Vec<BeeOccupant>,
    },
    /// The loot is not rolled here: the reference stores the table's id and its
    /// seed and rolls when a player first opens the chest.
    Chest(ContainerData),
    TrappedChest(ContainerData),
    EnderChest {
        x: i32,
        y: i32,
        z: i32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        components: Option<NbtCompound>,
    },
    Barrel(ContainerData),
    Dispenser(ContainerData),
    Hopper(HopperData),
    Furnace(FurnaceData),
    BlastFurnace(FurnaceData),
    Smoker(FurnaceData),
    BrewingStand {
        x: i32,
        y: i32,
        z: i32,
        #[serde(rename = "BrewTime", default)]
        brew_time: i32,
        #[serde(default = "brew_time_total")]
        total_brew_time: i32,
        #[serde(rename = "Items", default)]
        items: Vec<ItemStackWithSlot<Named>>,
        #[serde(rename = "Fuel", default)]
        fuel: i32,
        #[serde(default = "fuel_total")]
        total_fuel: i32,
        #[serde(default = "one_f32")]
        speed_multiplier: f32,
        #[serde(
            rename = "CustomName",
            default,
            skip_serializing_if = "Option::is_none"
        )]
        custom_name: Option<Text>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        components: Option<NbtCompound>,
    },
    Campfire {
        x: i32,
        y: i32,
        z: i32,
        #[serde(rename = "Items", default)]
        items: Vec<ItemStackWithSlot<Named>>,
        #[serde(rename = "CookingTimes", default, serialize_with = "nbt_int_array")]
        cooking_times: [i32; 4],
        #[serde(
            rename = "CookingTotalTimes",
            default,
            serialize_with = "nbt_int_array"
        )]
        cooking_total_times: [i32; 4],
        #[serde(default, skip_serializing_if = "Option::is_none")]
        components: Option<NbtCompound>,
    },
    Comparator {
        x: i32,
        y: i32,
        z: i32,
        #[serde(rename = "OutputSignal", default)]
        output_signal: i32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        components: Option<NbtCompound>,
    },
    Bell {
        x: i32,
        y: i32,
        z: i32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        components: Option<NbtCompound>,
    },
    CopperGolemStatue {
        x: i32,
        y: i32,
        z: i32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        components: Option<NbtCompound>,
    },
    Lectern {
        x: i32,
        y: i32,
        z: i32,
        #[serde(rename = "Book", default, skip_serializing_if = "Option::is_none")]
        book: Option<SavedItem>,
        #[serde(rename = "Page", default, skip_serializing_if = "Option::is_none")]
        page: Option<i32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        components: Option<NbtCompound>,
    },
    /// A jigsaw block a non-jigsaw piece leaves in the world, the way the
    /// ruined portals do; a jigsaw piece replaces its own before writing.
    Jigsaw {
        x: i32,
        y: i32,
        z: i32,
        #[serde(default = "empty_id")]
        name: String,
        #[serde(default = "empty_id")]
        target: String,
        #[serde(default = "empty_pool")]
        pool: String,
        #[serde(default = "air_id")]
        final_state: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        joint: Option<Joint>,
        #[serde(default)]
        placement_priority: i32,
        #[serde(default)]
        selection_priority: i32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        components: Option<NbtCompound>,
    },
    CreakingHeart {
        x: i32,
        y: i32,
        z: i32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        creaking: Option<Uuid>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        components: Option<NbtCompound>,
    },
    DecoratedPot {
        x: i32,
        y: i32,
        z: i32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        sherds: Option<NbtCompound>,
        #[serde(rename = "LootTable", default, skip_serializing_if = "Option::is_none")]
        loot_table: Option<String>,
        #[serde(rename = "LootTableSeed", default, skip_serializing_if = "is_default")]
        loot_table_seed: i64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        item: Option<SavedItem>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        components: Option<NbtCompound>,
    },
    BrushableBlock {
        x: i32,
        y: i32,
        z: i32,
        #[serde(rename = "LootTable", default, skip_serializing_if = "Option::is_none")]
        loot_table: Option<String>,
        #[serde(rename = "LootTableSeed", default, skip_serializing_if = "is_default")]
        loot_table_seed: i64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        item: Option<SavedItem>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        components: Option<NbtCompound>,
    },
    Banner {
        x: i32,
        y: i32,
        z: i32,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        patterns: Vec<BannerLayer>,
        #[serde(
            rename = "CustomName",
            default,
            skip_serializing_if = "Option::is_none"
        )]
        custom_name: Option<Text>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        components: Option<NbtCompound>,
    },
    Sign {
        x: i32,
        y: i32,
        z: i32,
        #[serde(default)]
        front_text: SignText,
        #[serde(default)]
        back_text: SignText,
        #[serde(default, deserialize_with = "nbt_flag")]
        is_waxed: bool,
        #[serde(
            default,
            skip_serializing_if = "std::ops::Not::not",
            deserialize_with = "nbt_flag"
        )]
        allow_op_features: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        components: Option<NbtCompound>,
    },
    Skull {
        x: i32,
        y: i32,
        z: i32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        profile: Option<NbtTag>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        note_block_sound: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        custom_name: Option<Text>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        components: Option<NbtCompound>,
    },
    SculkSensor {
        x: i32,
        y: i32,
        z: i32,
        #[serde(default)]
        last_vibration_frequency: i32,
        #[serde(default = "fresh_listener")]
        listener: NbtCompound,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        components: Option<NbtCompound>,
    },
    MobSpawner {
        x: i32,
        y: i32,
        z: i32,
        #[serde(rename = "Delay", default = "spawner_delay")]
        delay: i16,
        #[serde(rename = "MinSpawnDelay", default = "spawner_min_spawn_delay")]
        min_spawn_delay: i16,
        #[serde(rename = "MaxSpawnDelay", default = "spawner_max_spawn_delay")]
        max_spawn_delay: i16,
        #[serde(rename = "SpawnCount", default = "spawner_spawn_count")]
        spawn_count: i16,
        #[serde(rename = "MaxNearbyEntities", default = "spawner_max_nearby_entities")]
        max_nearby_entities: i16,
        #[serde(
            rename = "RequiredPlayerRange",
            default = "spawner_required_player_range"
        )]
        required_player_range: i16,
        #[serde(rename = "SpawnRange", default = "spawner_spawn_range")]
        spawn_range: i16,
        #[serde(rename = "SpawnData", default, skip_serializing_if = "Option::is_none")]
        spawn_data: Option<SpawnData>,
        #[serde(rename = "SpawnPotentials", default)]
        spawn_potentials: Vec<Weighted<SpawnData>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        components: Option<NbtCompound>,
    },
    TrialSpawner {
        x: i32,
        y: i32,
        z: i32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        normal_config: Option<NbtTag>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        ominous_config: Option<NbtTag>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        target_cooldown_length: Option<i32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        required_player_range: Option<i32>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        registered_players: Vec<Uuid>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        current_mobs: Vec<Uuid>,
        #[serde(default, skip_serializing_if = "is_default")]
        cooldown_ends_at: i64,
        #[serde(default, skip_serializing_if = "is_default")]
        next_mob_spawns_at: i64,
        #[serde(default, skip_serializing_if = "is_default")]
        total_mobs_spawned: i32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        spawn_data: Option<SpawnData>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        ejecting_loot_table: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        components: Option<NbtCompound>,
    },
    Vault {
        x: i32,
        y: i32,
        z: i32,
        #[serde(default)]
        config: VaultConfig,
        #[serde(default)]
        shared_data: NbtCompound,
        #[serde(default)]
        server_data: NbtCompound,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        components: Option<NbtCompound>,
    },
    EndGateway(EndGatewayData),
}

fn one_f32() -> f32 {
    1.0
}

fn brew_time_total() -> i32 {
    400
}

fn fuel_total() -> i32 {
    20
}

pub(crate) fn fresh_listener() -> NbtCompound {
    let mut selector = NbtCompound::new();
    selector.put_long("tick", -1);
    let mut listener = NbtCompound::new();
    listener.put_component("selector", selector);
    listener.put_int("event_delay", 0);
    listener
}

fn spawner_delay() -> i16 {
    20
}

fn spawner_min_spawn_delay() -> i16 {
    200
}

fn spawner_max_spawn_delay() -> i16 {
    800
}

fn spawner_spawn_count() -> i16 {
    4
}

fn spawner_max_nearby_entities() -> i16 {
    6
}

fn spawner_required_player_range() -> i16 {
    16
}

fn spawner_spawn_range() -> i16 {
    4
}

impl GeneratedBlockEntity {
    /// The kind a save entry's `id` names when a variant describes it.
    pub fn kind_of(id: &str) -> Option<BlockEntityType> {
        BlockEntityType::read(id).filter(|kind| Self::KINDS.contains(kind))
    }

    /// The kinds that are a `RandomizableContainer`: a template placing one
    /// draws its `LootTableSeed` from the placement random.
    pub const LOOT_SEEDED_IDS: [&'static str; 9] = [
        BlockEntityType::Chest.as_static_str(),
        BlockEntityType::TrappedChest.as_static_str(),
        BlockEntityType::Barrel.as_static_str(),
        BlockEntityType::Dispenser.as_static_str(),
        BlockEntityType::Dropper.as_static_str(),
        BlockEntityType::ShulkerBox.as_static_str(),
        BlockEntityType::Crafter.as_static_str(),
        BlockEntityType::Hopper.as_static_str(),
        BlockEntityType::DecoratedPot.as_static_str(),
    ];

    /// Which block the entity belongs to, which is what routes it to a column.
    pub fn position(&self) -> BlockPos {
        match self {
            GeneratedBlockEntity::Beehive { x, y, z, .. }
            | GeneratedBlockEntity::BrewingStand { x, y, z, .. }
            | GeneratedBlockEntity::Campfire { x, y, z, .. }
            | GeneratedBlockEntity::Comparator { x, y, z, .. }
            | GeneratedBlockEntity::Bell { x, y, z, .. }
            | GeneratedBlockEntity::EnderChest { x, y, z, .. }
            | GeneratedBlockEntity::CopperGolemStatue { x, y, z, .. }
            | GeneratedBlockEntity::Lectern { x, y, z, .. }
            | GeneratedBlockEntity::Jigsaw { x, y, z, .. }
            | GeneratedBlockEntity::CreakingHeart { x, y, z, .. }
            | GeneratedBlockEntity::DecoratedPot { x, y, z, .. }
            | GeneratedBlockEntity::BrushableBlock { x, y, z, .. }
            | GeneratedBlockEntity::Banner { x, y, z, .. }
            | GeneratedBlockEntity::Sign { x, y, z, .. }
            | GeneratedBlockEntity::Skull { x, y, z, .. }
            | GeneratedBlockEntity::SculkSensor { x, y, z, .. }
            | GeneratedBlockEntity::MobSpawner { x, y, z, .. }
            | GeneratedBlockEntity::TrialSpawner { x, y, z, .. }
            | GeneratedBlockEntity::Vault { x, y, z, .. } => BlockPos::new(*x, *y, *z),
            GeneratedBlockEntity::Chest(c)
            | GeneratedBlockEntity::TrappedChest(c)
            | GeneratedBlockEntity::Barrel(c)
            | GeneratedBlockEntity::Dispenser(c) => BlockPos::new(c.x, c.y, c.z),
            GeneratedBlockEntity::Hopper(h) => {
                BlockPos::new(h.container.x, h.container.y, h.container.z)
            }
            GeneratedBlockEntity::Furnace(f)
            | GeneratedBlockEntity::BlastFurnace(f)
            | GeneratedBlockEntity::Smoker(f) => BlockPos::new(f.x, f.y, f.z),
            GeneratedBlockEntity::EndGateway(gateway) => {
                BlockPos::new(gateway.x, gateway.y, gateway.z)
            }
        }
    }

    pub fn wants_loot_seed(nbt: &NbtCompound) -> bool {
        nbt.get_string("id")
            .is_some_and(|id| Self::LOOT_SEEDED_IDS.contains(&id))
    }

    // chisle: the compound is written to bytes and read back through the
    // serde deserializer; the upgrade is a Deserializer over NbtTag itself.
    pub fn from_compound(compound: &NbtCompound) -> Result<Self, mcrs_minecraft_nbt::Error> {
        let bytes = Nbt::new(String::new(), compound.clone()).write_unnamed()?;
        mcrs_minecraft_nbt::from_bytes_unnamed(Cursor::new(bytes))
    }

    /// A template's block nbt as the entity placed at `pos`. `Ok(None)` for an
    /// id this type does not describe. The seed is stored only beside a table,
    /// as the reference saves it.
    pub fn from_template(
        nbt: &NbtCompound,
        pos: BlockPos,
        loot_seed: Option<i64>,
    ) -> Result<Option<Self>, mcrs_minecraft_nbt::Error> {
        if nbt.get_string("id").and_then(Self::kind_of).is_none() {
            return Ok(None);
        }
        let mut compound = nbt.clone();
        let seed = loot_seed.filter(|_| nbt.get("LootTable").is_some());
        compound.child_tags.retain(|(key, _)| {
            !matches!(key.as_str(), "x" | "y" | "z") && (seed.is_none() || key != "LootTableSeed")
        });
        compound.put_int("x", pos.x);
        compound.put_int("y", pos.y);
        compound.put_int("z", pos.z);
        if let Some(seed) = seed {
            compound.put_long("LootTableSeed", seed);
        }
        let mut entity = Self::from_compound(&compound)?;
        if let GeneratedBlockEntity::Sign {
            front_text,
            back_text,
            ..
        } = &mut entity
        {
            for text in [front_text, back_text] {
                if text.filtered_messages.as_ref() == Some(&text.messages) {
                    text.filtered_messages = None;
                }
            }
        }
        Ok(Some(entity))
    }

    pub fn chest(pos: BlockPos, loot_table: String, loot_table_seed: i64) -> Self {
        GeneratedBlockEntity::Chest(ContainerData::looted(pos, loot_table, loot_table_seed))
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
            delay: spawner_delay(),
            min_spawn_delay: spawner_min_spawn_delay(),
            max_spawn_delay: spawner_max_spawn_delay(),
            spawn_count: spawner_spawn_count(),
            max_nearby_entities: spawner_max_nearby_entities(),
            required_player_range: spawner_required_player_range(),
            spawn_range: spawner_spawn_range(),
            spawn_data: Some(SpawnData {
                entity,
                custom_spawn_rules: None,
                equipment: None,
            }),
            spawn_potentials: Vec::new(),
            components: None,
        }
    }
}
