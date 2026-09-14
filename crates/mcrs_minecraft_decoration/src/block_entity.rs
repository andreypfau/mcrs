// ponytail: data components, item components, skull profiles, pot sherds,
// trial-spawner configs and vault runtime state pass through as raw NBT. The
// ceiling is that nothing can read them typed; the upgrade is a typed
// DataComponentMap once something does.

use std::collections::BTreeMap;
use std::io::Cursor;

use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_nbt::tag::NbtTag;
use mcrs_minecraft_nbt::{Nbt, nbt_int_array};
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
    Chest(ContainerData),
    #[serde(rename = "minecraft:barrel")]
    Barrel(ContainerData),
    #[serde(rename = "minecraft:dispenser")]
    Dispenser(ContainerData),
    #[serde(rename = "minecraft:hopper")]
    Hopper {
        x: i32,
        y: i32,
        z: i32,
        #[serde(rename = "LootTable", default, skip_serializing_if = "Option::is_none")]
        loot_table: Option<String>,
        #[serde(rename = "LootTableSeed", default, skip_serializing_if = "is_default")]
        loot_table_seed: i64,
        #[serde(rename = "Items", default, skip_serializing_if = "Vec::is_empty")]
        items: Vec<SlotItem>,
        #[serde(
            rename = "CustomName",
            default,
            skip_serializing_if = "Option::is_none"
        )]
        custom_name: Option<Text>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        components: Option<NbtCompound>,
        #[serde(rename = "TransferCooldown", default = "minus_one")]
        transfer_cooldown: i32,
    },
    #[serde(rename = "minecraft:furnace")]
    Furnace(FurnaceData),
    #[serde(rename = "minecraft:blast_furnace")]
    BlastFurnace(FurnaceData),
    #[serde(rename = "minecraft:smoker")]
    Smoker(FurnaceData),
    #[serde(rename = "minecraft:brewing_stand")]
    BrewingStand {
        x: i32,
        y: i32,
        z: i32,
        #[serde(rename = "BrewTime", default)]
        brew_time: i32,
        #[serde(default = "brew_time_total")]
        total_brew_time: i32,
        #[serde(rename = "Items", default, skip_serializing_if = "Vec::is_empty")]
        items: Vec<SlotItem>,
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
    #[serde(rename = "minecraft:campfire")]
    Campfire {
        x: i32,
        y: i32,
        z: i32,
        #[serde(rename = "Items", default, skip_serializing_if = "Vec::is_empty")]
        items: Vec<SlotItem>,
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
    #[serde(rename = "minecraft:comparator")]
    Comparator {
        x: i32,
        y: i32,
        z: i32,
        #[serde(rename = "OutputSignal", default)]
        output_signal: i32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        components: Option<NbtCompound>,
    },
    #[serde(rename = "minecraft:bell")]
    Bell {
        x: i32,
        y: i32,
        z: i32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        components: Option<NbtCompound>,
    },
    #[serde(rename = "minecraft:copper_golem_statue")]
    CopperGolemStatue {
        x: i32,
        y: i32,
        z: i32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        components: Option<NbtCompound>,
    },
    #[serde(rename = "minecraft:lectern")]
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
    #[serde(rename = "minecraft:creaking_heart")]
    CreakingHeart {
        x: i32,
        y: i32,
        z: i32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        creaking: Option<Uuid>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        components: Option<NbtCompound>,
    },
    #[serde(rename = "minecraft:decorated_pot")]
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
    #[serde(rename = "minecraft:brushable_block")]
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
    #[serde(rename = "minecraft:banner")]
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
    #[serde(rename = "minecraft:sign")]
    Sign {
        x: i32,
        y: i32,
        z: i32,
        front_text: SignText,
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
    #[serde(rename = "minecraft:skull")]
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
    #[serde(rename = "minecraft:sculk_sensor")]
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
    #[serde(rename = "minecraft:mob_spawner")]
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
    #[serde(rename = "minecraft:trial_spawner")]
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
    #[serde(rename = "minecraft:vault")]
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

fn one() -> i32 {
    1
}

fn one_f32() -> f32 {
    1.0
}

fn minus_one() -> i32 {
    -1
}

fn brew_time_total() -> i32 {
    400
}

fn fuel_total() -> i32 {
    20
}

fn black() -> String {
    "black".to_owned()
}

fn fresh_listener() -> NbtCompound {
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

const VAULT_LOOT: &str = "minecraft:chests/trial_chambers/reward";
const VAULT_ACTIVATION_RANGE: f64 = 4.0;
const VAULT_DEACTIVATION_RANGE: f64 = 4.5;

fn is_vault_loot(value: &String) -> bool {
    value == VAULT_LOOT
}

fn is_vault_activation_range(value: &f64) -> bool {
    *value == VAULT_ACTIVATION_RANGE
}

fn is_vault_deactivation_range(value: &f64) -> bool {
    *value == VAULT_DEACTIVATION_RANGE
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SavedItem {
    pub id: String,
    #[serde(default = "one")]
    pub count: i32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub components: Option<NbtCompound>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SlotItem {
    #[serde(rename = "Slot")]
    pub slot: i8,
    pub id: String,
    #[serde(default = "one")]
    pub count: i32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub components: Option<NbtCompound>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Uuid(#[serde(serialize_with = "nbt_int_array")] pub [i32; 4]);

/// A chat component: a bare string when it is nothing but literal text, the
/// full compound otherwise.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(untagged)]
pub enum Text {
    Plain(String),
    Rich(NbtTag),
}

impl<'de> Deserialize<'de> for Text {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(match <NbtTag as Deserialize>::deserialize(deserializer)? {
            NbtTag::String(text) => Text::Plain(text),
            NbtTag::Compound(compound) => match compound.child_tags.as_slice() {
                [(key, NbtTag::String(text))] if key == "text" => Text::Plain(text.clone()),
                _ => Text::Rich(NbtTag::Compound(compound)),
            },
            tag => Text::Rich(tag),
        })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ContainerData {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    #[serde(rename = "LootTable", default, skip_serializing_if = "Option::is_none")]
    pub loot_table: Option<String>,
    #[serde(rename = "LootTableSeed", default, skip_serializing_if = "is_default")]
    pub loot_table_seed: i64,
    #[serde(rename = "Items", default, skip_serializing_if = "Vec::is_empty")]
    pub items: Vec<SlotItem>,
    #[serde(
        rename = "CustomName",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub custom_name: Option<Text>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub components: Option<NbtCompound>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FurnaceData {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    #[serde(default)]
    pub cooking_time_spent: i32,
    #[serde(default)]
    pub cooking_total_time: i32,
    #[serde(default)]
    pub lit_time_remaining: i32,
    #[serde(default)]
    pub lit_total_time: i32,
    #[serde(default = "one_f32")]
    pub speed_multiplier: f32,
    #[serde(rename = "Items", default, skip_serializing_if = "Vec::is_empty")]
    pub items: Vec<SlotItem>,
    #[serde(
        rename = "RecipesUsed",
        default,
        skip_serializing_if = "BTreeMap::is_empty"
    )]
    pub recipes_used: BTreeMap<String, i32>,
    #[serde(
        rename = "CustomName",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub custom_name: Option<Text>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub components: Option<NbtCompound>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SignText {
    pub messages: Vec<Text>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filtered_messages: Option<Vec<Text>>,
    #[serde(default = "black")]
    pub color: String,
    #[serde(default, deserialize_with = "nbt_flag")]
    pub has_glowing_text: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BannerLayer {
    pub pattern: String,
    pub color: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SpawnData {
    pub entity: NbtCompound,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_spawn_rules: Option<NbtCompound>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub equipment: Option<NbtCompound>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct VaultConfig {
    #[serde(skip_serializing_if = "is_vault_loot")]
    pub loot_table: String,
    #[serde(skip_serializing_if = "is_vault_activation_range")]
    pub activation_range: f64,
    #[serde(skip_serializing_if = "is_vault_deactivation_range")]
    pub deactivation_range: f64,
    pub key_item: SavedItem,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub override_loot_table_to_display: Option<String>,
}

impl Default for VaultConfig {
    fn default() -> Self {
        VaultConfig {
            loot_table: VAULT_LOOT.to_owned(),
            activation_range: VAULT_ACTIVATION_RANGE,
            deactivation_range: VAULT_DEACTIVATION_RANGE,
            key_item: SavedItem {
                id: "minecraft:trial_key".to_owned(),
                count: 1,
                components: None,
            },
            override_loot_table_to_display: None,
        }
    }
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

impl GeneratedBlockEntity {
    /// The `id` each variant is tagged with; a save entry naming any other kind
    /// is one this type does not describe.
    pub const IDS: [&'static str; 25] = [
        "minecraft:beehive",
        "minecraft:chest",
        "minecraft:mob_spawner",
        "minecraft:end_gateway",
        "minecraft:barrel",
        "minecraft:dispenser",
        "minecraft:hopper",
        "minecraft:furnace",
        "minecraft:blast_furnace",
        "minecraft:smoker",
        "minecraft:brewing_stand",
        "minecraft:campfire",
        "minecraft:comparator",
        "minecraft:bell",
        "minecraft:copper_golem_statue",
        "minecraft:lectern",
        "minecraft:creaking_heart",
        "minecraft:decorated_pot",
        "minecraft:brushable_block",
        "minecraft:banner",
        "minecraft:sign",
        "minecraft:skull",
        "minecraft:sculk_sensor",
        "minecraft:trial_spawner",
        "minecraft:vault",
    ];

    /// The kinds that are a `RandomizableContainer`: a template placing one
    /// draws its `LootTableSeed` from the placement random.
    pub const LOOT_SEEDED_IDS: [&'static str; 5] = [
        "minecraft:chest",
        "minecraft:barrel",
        "minecraft:dispenser",
        "minecraft:hopper",
        "minecraft:decorated_pot",
    ];

    /// Which block the entity belongs to, which is what routes it to a column.
    pub fn position(&self) -> BlockPos {
        match self {
            GeneratedBlockEntity::Beehive { x, y, z, .. }
            | GeneratedBlockEntity::Hopper { x, y, z, .. }
            | GeneratedBlockEntity::BrewingStand { x, y, z, .. }
            | GeneratedBlockEntity::Campfire { x, y, z, .. }
            | GeneratedBlockEntity::Comparator { x, y, z, .. }
            | GeneratedBlockEntity::Bell { x, y, z, .. }
            | GeneratedBlockEntity::CopperGolemStatue { x, y, z, .. }
            | GeneratedBlockEntity::Lectern { x, y, z, .. }
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
            | GeneratedBlockEntity::Barrel(c)
            | GeneratedBlockEntity::Dispenser(c) => BlockPos::new(c.x, c.y, c.z),
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

    // ponytail: the compound is written to bytes and read back through the
    // serde deserializer; the upgrade is a Deserializer over NbtTag itself.
    pub fn from_compound(compound: &NbtCompound) -> Result<Self, mcrs_minecraft_nbt::Error> {
        let bytes = Nbt::new(String::new(), compound.clone()).write_unnamed();
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
        if !nbt
            .get_string("id")
            .is_some_and(|id| Self::IDS.contains(&id))
        {
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
        GeneratedBlockEntity::Chest(ContainerData {
            x: pos.x,
            y: pos.y,
            z: pos.z,
            loot_table: Some(loot_table),
            loot_table_seed,
            items: Vec::new(),
            custom_name: None,
            components: None,
        })
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

#[cfg(test)]
mod tests {
    use super::*;
    use mcrs_minecraft_nbt::to_nbt_compound;

    const POS: BlockPos = BlockPos::new(1, -2, 3);

    fn container(loot: Option<&str>) -> ContainerData {
        ContainerData {
            x: 1,
            y: -2,
            z: 3,
            loot_table: loot.map(str::to_owned),
            loot_table_seed: 0,
            items: vec![SlotItem {
                slot: 4,
                id: "minecraft:bread".to_owned(),
                count: 2,
                components: None,
            }],
            custom_name: Some(Text::Plain("box".to_owned())),
            components: None,
        }
    }

    fn furnace() -> FurnaceData {
        FurnaceData {
            x: 1,
            y: -2,
            z: 3,
            cooking_time_spent: 3,
            cooking_total_time: 200,
            lit_time_remaining: 0,
            lit_total_time: 0,
            speed_multiplier: 1.0,
            items: Vec::new(),
            recipes_used: BTreeMap::from([("minecraft:iron_ingot".to_owned(), 2)]),
            custom_name: None,
            components: None,
        }
    }

    fn sign_text(text: &str) -> SignText {
        SignText {
            messages: vec![
                Text::Plain(text.to_owned()),
                Text::Plain(String::new()),
                Text::Plain(String::new()),
                Text::Plain(String::new()),
            ],
            filtered_messages: None,
            color: "black".to_owned(),
            has_glowing_text: false,
        }
    }

    fn one_of_each() -> Vec<GeneratedBlockEntity> {
        use GeneratedBlockEntity::*;
        let mut sherds = NbtCompound::new();
        sherds.put_string("back", "minecraft:brick".to_owned());
        let mut profile = NbtCompound::new();
        profile.put_string("name", "Steve".to_owned());
        vec![
            Beehive {
                x: 1,
                y: -2,
                z: 3,
                bees: vec![BeeOccupant::bee(5)],
            },
            Chest(container(Some(
                "minecraft:chests/village/village_toolsmith",
            ))),
            Barrel(container(None)),
            Dispenser(container(None)),
            Hopper {
                x: 1,
                y: -2,
                z: 3,
                loot_table: None,
                loot_table_seed: 0,
                items: Vec::new(),
                custom_name: None,
                components: None,
                transfer_cooldown: -1,
            },
            Furnace(furnace()),
            BlastFurnace(furnace()),
            Smoker(furnace()),
            BrewingStand {
                x: 1,
                y: -2,
                z: 3,
                brew_time: 0,
                total_brew_time: 400,
                items: Vec::new(),
                fuel: 0,
                total_fuel: 20,
                speed_multiplier: 1.0,
                custom_name: None,
                components: None,
            },
            Campfire {
                x: 1,
                y: -2,
                z: 3,
                items: Vec::new(),
                cooking_times: [0, 1, 2, 3],
                cooking_total_times: [600; 4],
                components: None,
            },
            Comparator {
                x: 1,
                y: -2,
                z: 3,
                output_signal: 7,
                components: None,
            },
            Bell {
                x: 1,
                y: -2,
                z: 3,
                components: None,
            },
            CopperGolemStatue {
                x: 1,
                y: -2,
                z: 3,
                components: None,
            },
            Lectern {
                x: 1,
                y: -2,
                z: 3,
                book: Some(SavedItem {
                    id: "minecraft:written_book".to_owned(),
                    count: 1,
                    components: None,
                }),
                page: Some(2),
                components: None,
            },
            CreakingHeart {
                x: 1,
                y: -2,
                z: 3,
                creaking: Some(Uuid([1, 2, 3, 4])),
                components: None,
            },
            DecoratedPot {
                x: 1,
                y: -2,
                z: 3,
                sherds: Some(sherds),
                loot_table: Some("minecraft:archaeology/trail_ruins_common".to_owned()),
                loot_table_seed: 9,
                item: None,
                components: None,
            },
            BrushableBlock {
                x: 1,
                y: -2,
                z: 3,
                loot_table: Some("minecraft:archaeology/desert_well".to_owned()),
                loot_table_seed: -5,
                item: None,
                components: None,
            },
            Banner {
                x: 1,
                y: -2,
                z: 3,
                patterns: vec![BannerLayer {
                    pattern: "minecraft:rhombus".to_owned(),
                    color: "cyan".to_owned(),
                }],
                custom_name: None,
                components: None,
            },
            Sign {
                x: 1,
                y: -2,
                z: 3,
                front_text: sign_text("hello"),
                back_text: sign_text(""),
                is_waxed: true,
                allow_op_features: false,
                components: None,
            },
            Skull {
                x: 1,
                y: -2,
                z: 3,
                profile: Some(NbtTag::Compound(profile)),
                note_block_sound: None,
                custom_name: None,
                components: None,
            },
            SculkSensor {
                x: 1,
                y: -2,
                z: 3,
                last_vibration_frequency: 0,
                listener: fresh_listener(),
                components: None,
            },
            GeneratedBlockEntity::mob_spawner(POS, "minecraft:zombie"),
            TrialSpawner {
                x: 1,
                y: -2,
                z: 3,
                normal_config: Some(NbtTag::String("minecraft:trial_chamber/breeze".to_owned())),
                ominous_config: None,
                target_cooldown_length: None,
                required_player_range: None,
                registered_players: vec![Uuid([5, 6, 7, 8])],
                current_mobs: Vec::new(),
                cooldown_ends_at: 0,
                next_mob_spawns_at: 0,
                total_mobs_spawned: 0,
                spawn_data: None,
                ejecting_loot_table: None,
                components: None,
            },
            Vault {
                x: 1,
                y: -2,
                z: 3,
                config: VaultConfig::default(),
                shared_data: NbtCompound::new(),
                server_data: NbtCompound::new(),
                components: None,
            },
            EndGateway(EndGatewayData {
                x: 1,
                y: -2,
                z: 3,
                age: 4,
                exit_portal: Some([1, 2, 3]),
                exact_teleport: true,
            }),
        ]
    }

    #[test]
    fn every_kind_round_trips_and_is_named_by_its_id() {
        let all = one_of_each();
        assert_eq!(all.len(), GeneratedBlockEntity::IDS.len());
        for entity in all {
            let compound = to_nbt_compound(&entity).unwrap();
            let id = compound.get_string("id").unwrap();
            assert!(GeneratedBlockEntity::IDS.contains(&id), "{id}");
            assert_eq!(entity.position(), POS);
            assert_eq!(
                GeneratedBlockEntity::from_compound(&compound).unwrap(),
                entity,
                "{id}"
            );
        }
    }

    fn template_chest(with_table: bool) -> NbtCompound {
        let mut nbt = NbtCompound::new();
        nbt.put_string("id", "minecraft:chest".to_owned());
        if with_table {
            nbt.put_string(
                "LootTable",
                "minecraft:chests/village/village_armorer".to_owned(),
            );
        }
        nbt.put_list("Items", Vec::new());
        nbt
    }

    #[test]
    fn a_template_chest_takes_the_placement_seed_only_beside_a_table() {
        let seeded = GeneratedBlockEntity::from_template(&template_chest(true), POS, Some(77))
            .unwrap()
            .unwrap();
        assert_eq!(
            seeded,
            GeneratedBlockEntity::chest(
                POS,
                "minecraft:chests/village/village_armorer".to_owned(),
                77
            )
        );
        let unseeded = GeneratedBlockEntity::from_template(&template_chest(false), POS, Some(77))
            .unwrap()
            .unwrap();
        let compound = to_nbt_compound(&unseeded).unwrap();
        assert!(compound.get("LootTableSeed").is_none());
        assert!(compound.get("Items").is_none());
        assert_eq!(compound.get_int("x"), Some(1));
    }

    #[test]
    fn foreign_ids_are_not_entities_and_only_containers_want_a_seed() {
        let mut nbt = NbtCompound::new();
        nbt.put_string("id", "minecraft:structure_block".to_owned());
        assert_eq!(
            GeneratedBlockEntity::from_template(&nbt, POS, None).unwrap(),
            None
        );
        assert!(GeneratedBlockEntity::wants_loot_seed(&template_chest(true)));
        let mut brushable = NbtCompound::new();
        brushable.put_string("id", "minecraft:brushable_block".to_owned());
        assert!(!GeneratedBlockEntity::wants_loot_seed(&brushable));
        assert!(!GeneratedBlockEntity::wants_loot_seed(&nbt));
    }

    #[test]
    fn short_timers_load_as_ints_and_save_as_ints() {
        let mut nbt = NbtCompound::new();
        nbt.put_string("id", "minecraft:furnace".to_owned());
        nbt.put_short("cooking_time_spent", 12);
        nbt.put_byte("lit_total_time", 3);
        let entity = GeneratedBlockEntity::from_template(&nbt, POS, None)
            .unwrap()
            .unwrap();
        let GeneratedBlockEntity::Furnace(data) = &entity else {
            panic!("a furnace");
        };
        assert_eq!(data.cooking_time_spent, 12);
        assert_eq!(data.lit_total_time, 3);
        assert_eq!(data.speed_multiplier, 1.0);
        let compound = to_nbt_compound(&entity).unwrap();
        assert_eq!(compound.get("cooking_time_spent"), Some(&NbtTag::Int(12)));
        assert_eq!(compound.get("lit_total_time"), Some(&NbtTag::Int(3)));
        assert_eq!(compound.get("speed_multiplier"), Some(&NbtTag::Float(1.0)));
        assert!(compound.get("RecipesUsed").is_none());
    }

    #[test]
    fn text_collapses_a_bare_literal_and_keeps_a_styled_one() {
        let mut literal = NbtCompound::new();
        literal.put_string("text", "a".to_owned());
        let mut styled = literal.clone();
        styled.put_string("color", "red".to_owned());
        let mut nbt = NbtCompound::new();
        nbt.put_string("id", "minecraft:banner".to_owned());
        nbt.put_component("CustomName", literal);
        let banner = GeneratedBlockEntity::from_template(&nbt, POS, None)
            .unwrap()
            .unwrap();
        let GeneratedBlockEntity::Banner { custom_name, .. } = &banner else {
            panic!("a banner");
        };
        assert_eq!(custom_name, &Some(Text::Plain("a".to_owned())));
        let mut nbt = NbtCompound::new();
        nbt.put_string("id", "minecraft:banner".to_owned());
        nbt.put_component("CustomName", styled.clone());
        let banner = GeneratedBlockEntity::from_template(&nbt, POS, None)
            .unwrap()
            .unwrap();
        let GeneratedBlockEntity::Banner { custom_name, .. } = &banner else {
            panic!("a banner");
        };
        assert_eq!(
            custom_name,
            &Some(Text::Rich(NbtTag::Compound(styled.clone())))
        );
        assert_eq!(
            to_nbt_compound(&banner).unwrap().get("CustomName"),
            Some(&NbtTag::Compound(styled))
        );
    }

    #[test]
    fn defaults_fill_what_a_template_omits() {
        let load = |id: &str| {
            let mut nbt = NbtCompound::new();
            nbt.put_string("id", id.to_owned());
            GeneratedBlockEntity::from_template(&nbt, POS, None)
                .unwrap()
                .unwrap()
        };
        let GeneratedBlockEntity::BrewingStand {
            total_brew_time,
            total_fuel,
            speed_multiplier,
            ..
        } = load("minecraft:brewing_stand")
        else {
            panic!("a brewing stand");
        };
        assert_eq!(
            (total_brew_time, total_fuel, speed_multiplier),
            (400, 20, 1.0)
        );
        let GeneratedBlockEntity::SculkSensor { listener, .. } = load("minecraft:sculk_sensor")
        else {
            panic!("a sensor");
        };
        assert_eq!(listener, fresh_listener());
        assert_eq!(
            listener.get_compound("selector").unwrap().get("tick"),
            Some(&NbtTag::Long(-1))
        );
        let vault = load("minecraft:vault");
        let GeneratedBlockEntity::Vault { config, .. } = &vault else {
            panic!("a vault");
        };
        assert_eq!(config, &VaultConfig::default());
        let saved = to_nbt_compound(&vault).unwrap();
        let config = saved.get_compound("config").unwrap();
        assert!(config.get("loot_table").is_none());
        assert!(config.get("activation_range").is_none());
        assert_eq!(
            config.get_compound("key_item").unwrap().get_string("id"),
            Some("minecraft:trial_key")
        );
        assert_eq!(
            saved.get("shared_data"),
            Some(&NbtTag::Compound(NbtCompound::new()))
        );
        let GeneratedBlockEntity::Hopper {
            transfer_cooldown, ..
        } = load("minecraft:hopper")
        else {
            panic!("a hopper");
        };
        assert_eq!(transfer_cooldown, -1);
        let GeneratedBlockEntity::MobSpawner {
            delay,
            max_spawn_delay,
            spawn_data,
            ..
        } = load("minecraft:mob_spawner")
        else {
            panic!("a spawner");
        };
        assert_eq!((delay, max_spawn_delay, spawn_data), (20, 800, None));
    }

    #[test]
    fn uuids_are_int_arrays() {
        let heart = GeneratedBlockEntity::CreakingHeart {
            x: 1,
            y: -2,
            z: 3,
            creaking: Some(Uuid([-1, 2, -3, 4])),
            components: None,
        };
        let compound = to_nbt_compound(&heart).unwrap();
        assert_eq!(
            compound.get("creaking"),
            Some(&NbtTag::IntArray(vec![-1, 2, -3, 4]))
        );
        assert_eq!(
            GeneratedBlockEntity::from_compound(&compound).unwrap(),
            heart
        );
    }

    #[test]
    fn a_sign_drops_filtered_messages_equal_to_its_messages() {
        let mut front = sign_text("kept");
        front.filtered_messages = Some(front.messages.clone());
        let mut back = sign_text("shown");
        back.filtered_messages = Some(sign_text("hidden").messages);
        let sign = GeneratedBlockEntity::Sign {
            x: 0,
            y: 0,
            z: 0,
            front_text: front,
            back_text: back.clone(),
            is_waxed: false,
            allow_op_features: false,
            components: None,
        };
        let compound = to_nbt_compound(&sign).unwrap();
        assert_eq!(compound.get("is_waxed"), Some(&NbtTag::Byte(0)));
        assert!(compound.get("allow_op_features").is_none());
        let GeneratedBlockEntity::Sign {
            front_text,
            back_text,
            ..
        } = GeneratedBlockEntity::from_template(&compound, POS, None)
            .unwrap()
            .unwrap()
        else {
            panic!("a sign");
        };
        assert_eq!(front_text, sign_text("kept"));
        assert_eq!(back_text, back);
    }
}
