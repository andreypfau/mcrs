use std::io::Cursor;

use bevy_math::{DVec3, IVec3};
use mcrs_minecraft_core::mth::wrap_degrees;
use mcrs_minecraft_core::{BlockPos, ColumnPos, Direction, Mirror, ResourceLocation, Rotation};
use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_nbt::{Nbt, nbt_int_array};
use mcrs_minecraft_random::Random;
use mcrs_minecraft_random::worldgen::WorldgenRandom;
use mcrs_minecraft_random::xoroshiro::XoroshiroRandom;
pub use mcrs_minecraft_worldgen_feature::spawn_condition::{
    Condition, IdSet, SpawnContext, VariantTable, VariantTables,
};
use mcrs_minecraft_worldgen_feature::template::{EntityKind, FrozenEntity, VillagerData};
use serde::{Deserialize, Serialize};

use crate::block_entity::nbt_flag;

/// An entity a generator spawned, in the compound the save writes; a
/// projection of the seed until delivery makes it real.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GeneratedEntity {
    #[serde(rename = "Pos")]
    pub pos: [f64; 3],
    #[serde(rename = "Rotation")]
    pub rotation: [f32; 2],
    #[serde(rename = "UUID", serialize_with = "nbt_int_array")]
    pub uuid: [i32; 4],
    #[serde(flatten)]
    pub kind: GeneratedKind,
    #[serde(rename = "Passengers", default, skip_serializing_if = "Vec::is_empty")]
    pub passengers: Vec<GeneratedEntity>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "id")]
pub enum GeneratedKind {
    #[serde(rename = "minecraft:witch")]
    Witch {
        #[serde(rename = "LeftHanded", deserialize_with = "nbt_flag")]
        left_handed: bool,
    },
    #[serde(rename = "minecraft:cat")]
    Cat {
        #[serde(rename = "LeftHanded", deserialize_with = "nbt_flag")]
        left_handed: bool,
        variant: ResourceLocation,
        sound_variant: ResourceLocation,
    },
    #[serde(rename = "minecraft:elder_guardian")]
    ElderGuardian {
        #[serde(rename = "LeftHanded", deserialize_with = "nbt_flag")]
        left_handed: bool,
    },
    #[serde(rename = "minecraft:drowned")]
    Drowned {
        #[serde(rename = "LeftHanded", deserialize_with = "nbt_flag")]
        left_handed: bool,
        #[serde(rename = "IsBaby", deserialize_with = "nbt_flag")]
        baby: bool,
        #[serde(default, skip_serializing_if = "Equipment::is_empty")]
        equipment: Equipment,
    },
    #[serde(rename = "minecraft:chicken")]
    Chicken {
        #[serde(rename = "LeftHanded", deserialize_with = "nbt_flag")]
        left_handed: bool,
        #[serde(rename = "IsChickenJockey", deserialize_with = "nbt_flag")]
        jockey: bool,
        variant: ResourceLocation,
        sound_variant: ResourceLocation,
    },
    #[serde(rename = "minecraft:zombie_nautilus")]
    ZombieNautilus {
        #[serde(rename = "LeftHanded", deserialize_with = "nbt_flag")]
        left_handed: bool,
        variant: ResourceLocation,
    },
    #[serde(rename = "minecraft:shulker")]
    Shulker {
        #[serde(rename = "LeftHanded", deserialize_with = "nbt_flag")]
        left_handed: bool,
    },
    #[serde(rename = "minecraft:item_frame")]
    ItemFrame {
        #[serde(rename = "Item")]
        item: ItemStack,
        #[serde(
            rename = "Facing",
            serialize_with = "facing_id",
            deserialize_with = "facing_from_id"
        )]
        facing: Direction,
    },
    #[serde(rename = "minecraft:evoker")]
    Evoker {
        #[serde(rename = "LeftHanded", deserialize_with = "nbt_flag")]
        left_handed: bool,
    },
    #[serde(rename = "minecraft:vindicator")]
    Vindicator {
        #[serde(rename = "LeftHanded", deserialize_with = "nbt_flag")]
        left_handed: bool,
        #[serde(default, skip_serializing_if = "Equipment::is_empty")]
        equipment: Equipment,
    },
    #[serde(rename = "minecraft:allay")]
    Allay {
        #[serde(rename = "LeftHanded", deserialize_with = "nbt_flag")]
        left_handed: bool,
    },
    #[serde(rename = "minecraft:villager")]
    Villager {
        #[serde(rename = "VillagerData")]
        data: VillagerData,
    },
    #[serde(rename = "minecraft:zombie_villager")]
    ZombieVillager {
        #[serde(rename = "VillagerData")]
        data: VillagerData,
    },
    #[serde(rename = "minecraft:chest_minecart")]
    ChestMinecart {
        #[serde(rename = "LootTable")]
        loot_table: String,
        #[serde(rename = "LootTableSeed")]
        loot_table_seed: i64,
    },
}

impl GeneratedKind {
    pub const IDS: [&'static str; 14] = [
        "minecraft:witch",
        "minecraft:cat",
        "minecraft:elder_guardian",
        "minecraft:drowned",
        "minecraft:chicken",
        "minecraft:zombie_nautilus",
        "minecraft:shulker",
        "minecraft:item_frame",
        "minecraft:evoker",
        "minecraft:vindicator",
        "minecraft:allay",
        "minecraft:villager",
        "minecraft:zombie_villager",
        "minecraft:chest_minecart",
    ];

    pub fn id(&self) -> &'static str {
        match self {
            GeneratedKind::Witch { .. } => Self::IDS[0],
            GeneratedKind::Cat { .. } => Self::IDS[1],
            GeneratedKind::ElderGuardian { .. } => Self::IDS[2],
            GeneratedKind::Drowned { .. } => Self::IDS[3],
            GeneratedKind::Chicken { .. } => Self::IDS[4],
            GeneratedKind::ZombieNautilus { .. } => Self::IDS[5],
            GeneratedKind::Shulker { .. } => Self::IDS[6],
            GeneratedKind::ItemFrame { .. } => Self::IDS[7],
            GeneratedKind::Evoker { .. } => Self::IDS[8],
            GeneratedKind::Vindicator { .. } => Self::IDS[9],
            GeneratedKind::Allay { .. } => Self::IDS[10],
            GeneratedKind::Villager { .. } => Self::IDS[11],
            GeneratedKind::ZombieVillager { .. } => Self::IDS[12],
            GeneratedKind::ChestMinecart { .. } => Self::IDS[13],
        }
    }
}

/// The items the spawned kinds carry, by their registry ids.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Item {
    #[serde(rename = "minecraft:trident")]
    Trident,
    #[serde(rename = "minecraft:fishing_rod")]
    FishingRod,
    #[serde(rename = "minecraft:nautilus_shell")]
    NautilusShell,
    #[serde(rename = "minecraft:iron_axe")]
    IronAxe,
    #[serde(rename = "minecraft:elytra")]
    Elytra,
}

/// `ItemStack.CODEC`, whose `count` is optional to read and always written.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ItemStack {
    pub id: Item,
    #[serde(default = "crate::block_entity::one")]
    pub count: i32,
}

impl ItemStack {
    pub fn one(id: Item) -> Self {
        ItemStack { id, count: 1 }
    }
}

/// `Direction.LEGACY_ID_CODEC`: the face's 3D id as a byte.
fn facing_id<S: serde::Serializer>(facing: &Direction, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_i8(facing.id() as i8)
}

fn facing_from_id<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Direction, D::Error> {
    let id = i8::deserialize(deserializer)?;
    Direction::all()
        .into_iter()
        .find(|direction| direction.id() == id as usize)
        .ok_or_else(|| serde::de::Error::custom(format!("no direction has the id {id}")))
}

/// The slots a spawned mob can hold something in.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Equipment {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mainhand: Option<ItemStack>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offhand: Option<ItemStack>,
}

impl Equipment {
    pub fn is_empty(&self) -> bool {
        *self == Equipment::default()
    }
}

impl GeneratedEntity {
    pub fn column(&self) -> ColumnPos {
        ColumnPos::new(
            (self.pos[0].floor() as i32).div_euclid(16),
            (self.pos[2].floor() as i32).div_euclid(16),
        )
    }

    pub fn from_compound(compound: &NbtCompound) -> Result<Self, mcrs_minecraft_nbt::Error> {
        let bytes = Nbt::new(String::new(), compound.clone()).write_unnamed();
        mcrs_minecraft_nbt::from_bytes_unnamed(Cursor::new(bytes))
    }

    /// A template entity the piece places without finalizing: the two
    /// villager kinds, since no hardcoded structure spawns another.
    pub fn from_template(
        entity: &FrozenEntity,
        mirror: Mirror,
        rotation: Rotation,
        pivot: IVec3,
        position: IVec3,
        rng: &WorldgenRandom,
    ) -> Option<Self> {
        let kind = match &entity.kind {
            EntityKind::Villager { data } => GeneratedKind::Villager { data: data.clone() },
            EntityKind::ZombieVillager { data } => {
                GeneratedKind::ZombieVillager { data: data.clone() }
            }
            _ => return None,
        };
        let pos = mcrs_minecraft_worldgen_feature::template::transform_continuous(
            DVec3::from_array(entity.pos),
            mirror,
            rotation,
            pivot,
        ) + position.as_dvec3();
        let [yaw, pitch] = entity.rotation.map(|angle| angle % 360.0);
        let turned = wrap_degrees(yaw)
            + match rotation {
                Rotation::None => 0.0,
                Rotation::Clockwise90 => 90.0,
                Rotation::Clockwise180 => 180.0,
                Rotation::Counterclockwise90 => 270.0,
            };
        let mirrored = match mirror {
            Mirror::None => wrap_degrees(yaw),
            Mirror::FrontBack => -wrap_degrees(yaw),
            Mirror::LeftRight => 180.0 - wrap_degrees(yaw),
        };
        Some(placed(pos, [turned + (mirrored - yaw), pitch], kind, rng))
    }
}

/// The values an entity draws from its own random, which the reference seeds
/// from the clock: a fork of the placement stream by kind and position, so a
/// regenerated column spawns the same entity without moving the stream.
fn own_random(rng: &WorldgenRandom, id: &str, pos: DVec3) -> XoroshiroRandom {
    rng.source().clone().fork_hash(id).fork_at(IVec3::new(
        pos.x.floor() as i32,
        pos.y.floor() as i32,
        pos.z.floor() as i32,
    ))
}

/// `Mth.createInsecureUUID`.
fn uuid(own: &mut XoroshiroRandom) -> [i32; 4] {
    let most = own.next_i64() & -61441 | 16384;
    let least = own.next_i64() & 4611686018427387903 | i64::MIN;
    [
        (most >> 32) as i32,
        most as i32,
        (least >> 32) as i32,
        least as i32,
    ]
}

fn placed(
    pos: DVec3,
    rotation: [f32; 2],
    kind: GeneratedKind,
    rng: &WorldgenRandom,
) -> GeneratedEntity {
    let mut own = own_random(rng, kind.id(), pos);
    GeneratedEntity {
        pos: pos.to_array(),
        rotation,
        uuid: uuid(&mut own),
        kind,
        passengers: Vec::new(),
    }
}

/// `Entity.snapTo(pos, 0, 0)` at a block's bottom centre.
fn bottom_centre(at: BlockPos) -> DVec3 {
    DVec3::new(
        f64::from(at.x) + 0.5,
        f64::from(at.y),
        f64::from(at.z) + 0.5,
    )
}

/// `Mob.finalizeSpawn`: the follow-range bonus, which nothing static reads,
/// and the left hand.
fn mob(rng: &mut WorldgenRandom) -> bool {
    rng.next_f64();
    rng.next_f64();
    rng.next_f32() < 0.05
}

/// `enchantSpawnedEquipment` on a held item: one draw that never passes at
/// special multiplier zero.
fn enchant_held(rng: &mut WorldgenRandom) {
    rng.next_f32();
}

pub fn witch(at: BlockPos, rng: &mut WorldgenRandom) -> GeneratedEntity {
    let left_handed = mob(rng);
    placed(
        bottom_centre(at),
        [0.0; 2],
        GeneratedKind::Witch { left_handed },
        rng,
    )
}

pub fn elder_guardian(at: BlockPos, rng: &mut WorldgenRandom) -> GeneratedEntity {
    let left_handed = mob(rng);
    placed(
        bottom_centre(at),
        [0.0; 2],
        GeneratedKind::ElderGuardian { left_handed },
        rng,
    )
}

pub fn evoker(at: BlockPos, rng: &mut WorldgenRandom) -> GeneratedEntity {
    let left_handed = mob(rng);
    placed(
        bottom_centre(at),
        [0.0; 2],
        GeneratedKind::Evoker { left_handed },
        rng,
    )
}

pub fn vindicator(at: BlockPos, rng: &mut WorldgenRandom) -> GeneratedEntity {
    let left_handed = mob(rng);
    let equipment = Equipment {
        mainhand: Some(ItemStack::one(Item::IronAxe)),
        offhand: None,
    };
    enchant_held(rng);
    placed(
        bottom_centre(at),
        [0.0; 2],
        GeneratedKind::Vindicator {
            left_handed,
            equipment,
        },
        rng,
    )
}

/// The mansion's "Group of Allays" marker: one to three, counted on the
/// placement stream.
pub fn allays(at: BlockPos, rng: &mut WorldgenRandom) -> Vec<GeneratedEntity> {
    let count = rng.next_i32_bound(3) + 1;
    (0..count)
        .map(|_| {
            let left_handed = mob(rng);
            placed(
                bottom_centre(at),
                [0.0; 2],
                GeneratedKind::Allay { left_handed },
                rng,
            )
        })
        .collect()
}

/// An end city sentry: nothing is drawn, and the yaw is the one every living
/// entity rolls at construction, which no heading overrides here.
pub fn shulker(at: BlockPos, rng: &WorldgenRandom) -> GeneratedEntity {
    let pos = bottom_centre(at);
    let kind = GeneratedKind::Shulker { left_handed: false };
    let mut own = own_random(rng, kind.id(), pos);
    let uuid = uuid(&mut own);
    let yaw = own.next_f32() * std::f32::consts::TAU;
    GeneratedEntity {
        pos: pos.to_array(),
        rotation: [yaw, 0.0],
        uuid,
        kind,
        passengers: Vec::new(),
    }
}

/// `new ItemFrame(level, pos, facing)` holding an elytra: the frame hangs at
/// the block's centre pulled back to the wall behind it.
pub fn elytra_frame(at: BlockPos, facing: Direction, rng: &WorldgenRandom) -> GeneratedEntity {
    const SHIFT_TO_BLOCK_WALL: f64 = 0.46875;
    let pos = DVec3::new(
        f64::from(at.x) + 0.5,
        f64::from(at.y) + 0.5,
        f64::from(at.z) + 0.5,
    ) - facing.normal().as_dvec3() * SHIFT_TO_BLOCK_WALL;
    let (yaw, pitch) = match facing {
        Direction::Down => (0.0, 90.0),
        Direction::Up => (0.0, -90.0),
        Direction::North => (180.0, 0.0),
        Direction::South => (0.0, 0.0),
        Direction::West => (90.0, 0.0),
        Direction::East => (270.0, 0.0),
    };
    placed(
        pos,
        [yaw, pitch],
        GeneratedKind::ItemFrame {
            item: ItemStack::one(Item::Elytra),
            facing,
        },
        rng,
    )
}

/// A mineshaft's chest minecart at the block centre, seeded off the stream.
pub fn chest_minecart(
    at: BlockPos,
    loot_table: String,
    rng: &mut WorldgenRandom,
) -> GeneratedEntity {
    let pos = DVec3::new(
        f64::from(at.x) + 0.5,
        f64::from(at.y) + 0.5,
        f64::from(at.z) + 0.5,
    );
    let loot_table_seed = rng.next_java_long();
    placed(
        pos,
        [0.0; 2],
        GeneratedKind::ChestMinecart {
            loot_table,
            loot_table_seed,
        },
        rng,
    )
}

/// `Registry.getRandom` over a registry in its order.
fn pick_sound(sounds: &[ResourceLocation], rng: &mut WorldgenRandom) -> ResourceLocation {
    sounds[rng.next_i32_bound(sounds.len() as i32) as usize].clone()
}

pub fn cat(
    at: BlockPos,
    ctx: &SpawnContext,
    tables: &VariantTables,
    rng: &mut WorldgenRandom,
) -> GeneratedEntity {
    let left_handed = mob(rng);
    let variant = tables
        .cats
        .pick(ctx, rng)
        .cloned()
        .unwrap_or_else(|| ResourceLocation::minecraft("black"));
    let sound_variant = pick_sound(&tables.cat_sounds, rng);
    placed(
        bottom_centre(at),
        [0.0; 2],
        GeneratedKind::Cat {
            left_handed,
            variant,
            sound_variant,
        },
        rng,
    )
}

fn chicken_jockey(
    pos: DVec3,
    ctx: &SpawnContext,
    tables: &VariantTables,
    rng: &mut WorldgenRandom,
) -> GeneratedEntity {
    let variant = tables
        .chickens
        .pick(ctx, rng)
        .cloned()
        .unwrap_or_else(|| ResourceLocation::minecraft("temperate"));
    let sound_variant = pick_sound(&tables.chicken_sounds, rng);
    let left_handed = mob(rng);
    placed(
        pos,
        [0.0; 2],
        GeneratedKind::Chicken {
            left_handed,
            jockey: true,
            variant,
            sound_variant,
        },
        rng,
    )
}

fn zombie_nautilus(
    pos: DVec3,
    ctx: &SpawnContext,
    tables: &VariantTables,
    rng: &mut WorldgenRandom,
) -> GeneratedEntity {
    let variant = tables
        .zombie_nautiluses
        .pick(ctx, rng)
        .cloned()
        .unwrap_or_else(|| ResourceLocation::minecraft("temperate"));
    // `NautilusAi.initMemories`: the attack cooldown, `UniformInt.of(2400, 3600)`.
    rng.next_i32_bound(1201);
    let left_handed = mob(rng);
    placed(
        pos,
        [0.0; 2],
        GeneratedKind::ZombieNautilus {
            left_handed,
            variant,
        },
        rng,
    )
}

/// `Drowned.finalizeSpawn` from a ruin's marker. A drowned that mounts a
/// chicken or a zombie nautilus comes back as the mount with the drowned
/// riding it, as the save carries the pair. `frequent_drowned` is the
/// biome's `#more_frequent_drowned_spawns` membership, which forbids the
/// nautilus.
pub fn drowned(
    at: BlockPos,
    ctx: &SpawnContext,
    tables: &VariantTables,
    frequent_drowned: bool,
    rng: &mut WorldgenRandom,
) -> GeneratedEntity {
    let pos = bottom_centre(at);
    let left_handed = mob(rng);
    rng.next_f32();
    let baby = rng.next_f32() < 0.05;
    let mut mount = None;
    if baby {
        // A chicken already standing within five blocks would be ridden
        // instead; a generated column has none.
        let ridden_nearby = f64::from(rng.next_f32()) < 0.05;
        if !ridden_nearby && f64::from(rng.next_f32()) < 0.05 {
            mount = Some(chicken_jockey(pos, ctx, tables, rng));
        }
    }
    rng.next_f32();
    let mut equipment = Equipment::default();
    if f64::from(rng.next_f32()) > 0.9 {
        equipment.mainhand = Some(ItemStack::one(if rng.next_i32_bound(16) < 10 {
            Item::Trident
        } else {
            Item::FishingRod
        }));
        enchant_held(rng);
    }
    if rng.next_f32() < 0.03 {
        equipment.offhand = Some(ItemStack::one(Item::NautilusShell));
    }
    if equipment
        .mainhand
        .is_some_and(|held| held.id == Item::Trident)
        && rng.next_f32() < 0.5
        && !baby
        && !frequent_drowned
    {
        mount = Some(zombie_nautilus(pos, ctx, tables, rng));
    }
    let drowned = placed(
        pos,
        [0.0; 2],
        GeneratedKind::Drowned {
            left_handed,
            baby,
            equipment,
        },
        rng,
    );
    match mount {
        None => drowned,
        Some(mut mount) => {
            mount.passengers.push(drowned);
            mount
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fixedbitset::FixedBitSet;
    use mcrs_minecraft_core::HolderSet;
    use mcrs_minecraft_nbt::to_nbt_compound;
    use mcrs_minecraft_worldgen_feature::spawn_condition::{SpawnCondition, SpawnSelector};
    use std::sync::Arc;

    const AT: BlockPos = BlockPos::new(10, 64, -20);

    fn rng() -> WorldgenRandom {
        WorldgenRandom::new(12345)
    }

    fn draws(before: &WorldgenRandom, after: &WorldgenRandom) -> usize {
        let mut probe = before.clone();
        for count in 0..64 {
            if probe == *after {
                return count;
            }
            probe.next_i32();
        }
        panic!("more than 64 draws");
    }

    fn set(ids: &[usize]) -> IdSet {
        let mut bits = FixedBitSet::with_capacity(64);
        for id in ids {
            bits.insert(*id);
        }
        Arc::new(bits)
    }

    fn ids(names: &[&str]) -> Vec<ResourceLocation> {
        names
            .iter()
            .map(|n| ResourceLocation::minecraft(n))
            .collect()
    }

    #[derive(Deserialize)]
    struct Variant {
        spawn_conditions: Vec<SpawnSelector>,
    }

    fn tagged(
        tag: &'static str,
        ids: &'static [usize],
    ) -> impl Fn(&HolderSet) -> Result<IdSet, String> {
        move |set| match set {
            HolderSet::Tag(t) if t.path() == tag => Ok(self::set(ids)),
            other => Err(format!("{other:?} is not #{tag}")),
        }
    }

    fn tables() -> VariantTables {
        let dir = mcrs_minecraft_worldgen_testing::assets_dir().join("minecraft/cat_variant");
        let cats: Vec<(ResourceLocation, Vec<SpawnSelector>)> =
            mcrs_minecraft_worldgen_testing::json_files(&dir)
                .into_iter()
                .map(|path| {
                    let name = path.file_stem().unwrap().to_str().unwrap().to_owned();
                    let variant: Variant =
                        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
                    (ResourceLocation::minecraft(&name), variant.spawn_conditions)
                })
                .collect();
        assert_eq!(cats.len(), 11);
        let cats = VariantTable::freeze(
            cats.iter().map(|(id, s)| (id.clone(), s.as_slice())),
            &tagged("cats_spawn_as_black", &[7]),
            &tagged("", &[]),
        )
        .unwrap();
        let in_tag = |tag: &str| SpawnSelector {
            condition: Some(SpawnCondition::Biome {
                biomes: HolderSet::Tag(ResourceLocation::minecraft(tag)),
            }),
            priority: 1,
        };
        let plain = SpawnSelector {
            condition: None,
            priority: 0,
        };
        let chickens = VariantTable::freeze(
            [
                (
                    ResourceLocation::minecraft("cold"),
                    std::slice::from_ref(&in_tag("cold")),
                ),
                (
                    ResourceLocation::minecraft("temperate"),
                    std::slice::from_ref(&plain),
                ),
            ],
            &tagged("", &[]),
            &tagged("cold", &[5]),
        )
        .unwrap();
        let zombie_nautiluses = VariantTable::freeze(
            [
                (
                    ResourceLocation::minecraft("temperate"),
                    std::slice::from_ref(&plain),
                ),
                (
                    ResourceLocation::minecraft("warm"),
                    std::slice::from_ref(&in_tag("coral")),
                ),
            ],
            &tagged("", &[]),
            &tagged("coral", &[3]),
        )
        .unwrap();
        VariantTables {
            cats,
            cat_sounds: ids(&["classic", "royal"]),
            chickens,
            chicken_sounds: ids(&["classic", "picky"]),
            zombie_nautiluses,
        }
    }

    fn ctx(structure: Option<u32>, biome: u32) -> SpawnContext {
        SpawnContext {
            structure,
            biome,
            moon_brightness: 1.0,
        }
    }

    #[test]
    fn a_mob_draws_the_follow_range_bonus_then_its_hand() {
        let spawns: [fn(BlockPos, &mut WorldgenRandom) -> GeneratedEntity; 3] =
            [witch, elder_guardian, evoker];
        for spawn in spawns {
            let before = rng();
            let mut r = before.clone();
            let entity = spawn(AT, &mut r);
            assert_eq!(draws(&before, &r), 5);
            assert_eq!(entity.pos, [10.5, 64.0, -19.5]);
            assert_eq!(entity.rotation, [0.0, 0.0]);
        }
        let mut r = rng();
        let mut left = 0;
        for _ in 0..2000 {
            if let GeneratedKind::Witch { left_handed } = witch(AT, &mut r).kind {
                left += usize::from(left_handed);
            }
        }
        assert!((60..140).contains(&left), "{left} left-handed of 2000");
    }

    #[test]
    fn the_allay_count_is_drawn_first_then_each_allay() {
        let mut counts = [0; 4];
        let mut r = rng();
        for _ in 0..300 {
            let before = r.clone();
            let group = allays(AT, &mut r);
            assert_eq!(draws(&before, &r), 1 + 5 * group.len());
            counts[group.len()] += 1;
        }
        assert_eq!(counts[0], 0);
        assert!(counts[1..].iter().all(|c| *c > 50), "{counts:?}");
    }

    #[test]
    fn a_vindicator_holds_an_axe_and_rolls_its_enchantment() {
        let before = rng();
        let mut r = before.clone();
        let entity = vindicator(AT, &mut r);
        assert_eq!(draws(&before, &r), 6);
        let GeneratedKind::Vindicator { equipment, .. } = entity.kind else {
            panic!()
        };
        assert_eq!(equipment.mainhand, Some(ItemStack::one(Item::IronAxe)));
        assert_eq!(equipment.offhand, None);
    }

    #[test]
    fn a_cat_in_a_swamp_hut_is_all_black_after_one_draw() {
        let tables = tables();
        let before = rng();
        let mut r = before.clone();
        let entity = cat(AT, &ctx(Some(7), 0), &tables, &mut r);
        assert_eq!(draws(&before, &r), 7);
        let GeneratedKind::Cat {
            variant,
            sound_variant,
            ..
        } = entity.kind
        else {
            panic!()
        };
        assert_eq!(variant, ResourceLocation::minecraft("all_black"));
        assert!(tables.cat_sounds.contains(&sound_variant));

        let mut seen = std::collections::BTreeSet::new();
        for _ in 0..500 {
            if let GeneratedKind::Cat { variant, .. } = cat(AT, &ctx(None, 0), &tables, &mut r).kind
            {
                seen.insert(variant.path().to_owned());
            }
        }
        assert_eq!(seen.len(), 11, "a full moon admits every cat: {seen:?}");
        let dark = SpawnContext {
            moon_brightness: 0.5,
            ..ctx(None, 0)
        };
        let mut seen = std::collections::BTreeSet::new();
        for _ in 0..500 {
            if let GeneratedKind::Cat { variant, .. } = cat(AT, &dark, &tables, &mut r).kind {
                seen.insert(variant.path().to_owned());
            }
        }
        assert_eq!(seen.len(), 10);
        assert!(!seen.contains("all_black"));
    }

    fn drowned_after(r: &mut WorldgenRandom, frequent: bool) -> (GeneratedEntity, usize) {
        let before = r.clone();
        let entity = drowned(AT, &ctx(None, 3), &tables(), frequent, r);
        (entity, draws(&before, r))
    }

    #[test]
    fn a_bare_drowned_draws_the_zombie_sequence() {
        let mut r = rng();
        let mut bare = None;
        for _ in 0..200 {
            let (entity, count) = drowned_after(&mut r, false);
            if let GeneratedKind::Drowned {
                baby: false,
                equipment,
                ..
            } = &entity.kind
                && equipment.is_empty()
            {
                bare = Some(count);
                break;
            }
        }
        assert_eq!(bare, Some(10), "mob 5, pickup, baby, doors, gear, shell");
    }

    #[test]
    fn a_trident_drowned_may_ride_a_nautilus_unless_drowned_are_frequent() {
        let mut r = rng();
        let mut mounted = 0;
        let mut armed = 0;
        for _ in 0..3000 {
            let (entity, _) = drowned_after(&mut r, false);
            match &entity.kind {
                GeneratedKind::ZombieNautilus { variant, .. } => {
                    mounted += 1;
                    assert_eq!(variant, &ResourceLocation::minecraft("warm"));
                    let [rider] = entity.passengers.as_slice() else {
                        panic!()
                    };
                    let GeneratedKind::Drowned {
                        baby, equipment, ..
                    } = &rider.kind
                    else {
                        panic!()
                    };
                    assert!(!baby);
                    assert_eq!(equipment.mainhand, Some(ItemStack::one(Item::Trident)));
                    assert_eq!(rider.pos, entity.pos);
                }
                GeneratedKind::Drowned { equipment, .. }
                    if equipment.mainhand == Some(ItemStack::one(Item::Trident)) =>
                {
                    armed += 1
                }
                _ => {}
            }
        }
        assert!(
            mounted > 40 && armed > 40,
            "{mounted} mounted, {armed} on foot"
        );
        for _ in 0..3000 {
            let (entity, _) = drowned_after(&mut r, true);
            assert!(!matches!(entity.kind, GeneratedKind::ZombieNautilus { .. }));
        }
    }

    #[test]
    fn a_baby_drowned_may_ride_a_chicken_of_the_biome() {
        let mut r = rng();
        let mut jockeys = 0;
        let tables = tables();
        for _ in 0..40000 {
            let entity = drowned(AT, &ctx(None, 5), &tables, false, &mut r);
            if let GeneratedKind::Chicken {
                jockey, variant, ..
            } = &entity.kind
            {
                jockeys += 1;
                assert!(jockey);
                assert_eq!(variant, &ResourceLocation::minecraft("cold"));
                let GeneratedKind::Drowned { baby, .. } = entity.passengers[0].kind else {
                    panic!()
                };
                assert!(baby);
            }
        }
        assert!((40..160).contains(&jockeys), "{jockeys} jockeys of 40000");
    }

    #[test]
    fn a_shulker_and_a_frame_draw_nothing_and_a_minecart_draws_its_seed() {
        let before = rng();
        let sentry = shulker(AT, &before);
        assert!(matches!(sentry.kind, GeneratedKind::Shulker { .. }));
        assert!((0.0..std::f32::consts::TAU).contains(&sentry.rotation[0]));
        assert_eq!(
            shulker(AT, &before),
            sentry,
            "the yaw is a function of the stream"
        );
        assert_ne!(
            shulker(BlockPos::new(11, 64, -20), &before).uuid,
            sentry.uuid
        );

        let frame = elytra_frame(AT, Direction::West, &before);
        assert_eq!(frame.pos, [10.96875, 64.5, -19.5]);
        assert_eq!(frame.rotation, [90.0, 0.0]);

        let mut r = before.clone();
        let cart = chest_minecart(
            AT,
            "minecraft:chests/abandoned_mineshaft".to_owned(),
            &mut r,
        );
        assert_eq!(draws(&before, &r), 2);
        let mut probe = before.clone();
        assert_eq!(
            cart.kind,
            GeneratedKind::ChestMinecart {
                loot_table: "minecraft:chests/abandoned_mineshaft".to_owned(),
                loot_table_seed: probe.next_java_long(),
            }
        );
    }

    #[test]
    fn a_template_villager_turns_with_the_piece() {
        let frozen = FrozenEntity {
            pos: [2.5, 1.0, 1.5],
            block_pos: [2, 1, 1],
            rotation: [-4.5, -3.4],
            kind: EntityKind::Villager {
                data: VillagerData::default(),
            },
        };
        let r = rng();
        let turned = GeneratedEntity::from_template(
            &frozen,
            Mirror::None,
            Rotation::Clockwise90,
            IVec3::ZERO,
            IVec3::new(100, 60, 200),
            &r,
        )
        .unwrap();
        assert_eq!(turned.pos, [100.0 - 1.5 + 1.0, 61.0, 200.0 + 2.5]);
        assert_eq!(turned.rotation, [85.5, -3.4]);
        let mirrored = GeneratedEntity::from_template(
            &frozen,
            Mirror::LeftRight,
            Rotation::None,
            IVec3::ZERO,
            IVec3::ZERO,
            &r,
        )
        .unwrap();
        assert_eq!(mirrored.pos, [2.5, 1.0, -0.5]);
        assert_eq!(mirrored.rotation, [184.5, -3.4]);
        let camel = FrozenEntity {
            kind: EntityKind::Camel,
            ..frozen
        };
        assert!(
            GeneratedEntity::from_template(
                &camel,
                Mirror::None,
                Rotation::None,
                IVec3::ZERO,
                IVec3::ZERO,
                &r
            )
            .is_none()
        );
    }

    #[test]
    fn every_kind_round_trips_through_nbt_under_its_id() {
        let tables = tables();
        let mut r = rng();
        let mut all = vec![
            witch(AT, &mut r),
            cat(AT, &ctx(Some(7), 0), &tables, &mut r),
            elder_guardian(AT, &mut r),
            shulker(AT, &r),
            elytra_frame(AT, Direction::South, &r),
            evoker(AT, &mut r),
            vindicator(AT, &mut r),
            allays(AT, &mut r).remove(0),
            chest_minecart(
                AT,
                "minecraft:chests/abandoned_mineshaft".to_owned(),
                &mut r,
            ),
            chicken_jockey(bottom_centre(AT), &ctx(None, 0), &tables, &mut r),
            zombie_nautilus(bottom_centre(AT), &ctx(None, 0), &tables, &mut r),
        ];
        all.push(drowned(AT, &ctx(None, 0), &tables, false, &mut r));
        for kind in [
            GeneratedKind::Villager {
                data: VillagerData::default(),
            },
            GeneratedKind::ZombieVillager {
                data: VillagerData::default(),
            },
        ] {
            all.push(placed(bottom_centre(AT), [0.0; 2], kind, &r));
        }
        let mut nautilus = zombie_nautilus(bottom_centre(AT), &ctx(None, 0), &tables, &mut r);
        nautilus.passengers.push(witch(AT, &mut r));
        all.push(nautilus);
        let mut seen = std::collections::BTreeSet::new();
        for entity in all {
            let compound = to_nbt_compound(&entity).unwrap();
            let id = compound.get_string("id").unwrap();
            assert!(GeneratedKind::IDS.contains(&id), "{id}");
            assert_eq!(entity.kind.id(), id);
            seen.insert(id.to_owned());
            assert_eq!(
                GeneratedEntity::from_compound(&compound).unwrap(),
                entity,
                "{id}"
            );
        }
        assert_eq!(seen.len(), GeneratedKind::IDS.len());
    }

    #[test]
    fn a_uuid_carries_the_version_and_variant_bits() {
        let entity = witch(AT, &mut rng());
        let most = (i64::from(entity.uuid[0]) << 32) | i64::from(entity.uuid[1] as u32);
        let least = (i64::from(entity.uuid[2]) << 32) | i64::from(entity.uuid[3] as u32);
        assert_eq!((most >> 12) & 0xF, 4);
        assert_eq!((least >> 62) & 0x3, 0b10);
        assert_eq!(entity.column(), ColumnPos::new(0, -2));
    }
}
