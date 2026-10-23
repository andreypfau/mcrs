pub trait Sample: Sized {
    fn samples() -> Vec<Self>;

    /// The NBT tag id each dotted path below this sample's persistent form
    /// must carry, `""` naming the root; a width vanilla would write
    /// differently is a persistence bug the round trip alone cannot see.
    fn nbt_tags(&self) -> Vec<(&'static str, u8)> {
        Vec::new()
    }
}

use mcrs_minecraft_core::Registered;
use mcrs_minecraft_core::ResourceKey;
use std::sync::LazyLock;

use mcrs_minecraft_core::Id;
use mcrs_minecraft_core::codec::IntArray;
use mcrs_minecraft_core::{RegistryKey, ResourceLocation, rl};
use mcrs_minecraft_core::{TagId, TagKey};
use mcrs_minecraft_nbt::{COMPOUND_ID, INT_ARRAY_ID, LIST_ID, STRING_ID};
use mcrs_minecraft_profile::{
    GameProfileValue, PlayerModelType, PlayerName, Profile, ProfileIdentity, Property, SkinPatch,
    ints_uuid,
};
use mcrs_minecraft_registries::{IdSet, Namer, Resolver};
use std::any::TypeId;
use std::collections::HashMap;

impl Sample for Profile {
    fn nbt_tags(&self) -> Vec<(&'static str, u8)> {
        let mut tags = vec![("", COMPOUND_ID)];
        let properties = match &self.profile {
            ProfileIdentity::Full(profile) => {
                tags.extend([("id", INT_ARRAY_ID), ("name", STRING_ID)]);
                &profile.properties
            }
            ProfileIdentity::Partial {
                name,
                id,
                properties,
            } => {
                if name.is_some() {
                    tags.push(("name", STRING_ID));
                }
                if id.is_some() {
                    tags.push(("id", INT_ARRAY_ID));
                }
                properties
            }
        };
        if !properties.is_empty() {
            tags.push(("properties", LIST_ID));
        }
        if self.skin.model.is_some() {
            tags.push(("model", STRING_ID));
        }
        tags
    }

    fn samples() -> Vec<Self> {
        let property = |name: &str, value: &str, signature: Option<&str>| Property {
            name: name.into(),
            value: value.into(),
            signature: signature.map(Into::into),
        };
        vec![
            Profile::named("Notch").unwrap(),
            Profile {
                profile: ProfileIdentity::Full(GameProfileValue {
                    id: ints_uuid(IntArray([-1, 2, -3, 4])),
                    name: PlayerName::new("Steve").unwrap(),
                    properties: vec![
                        property("textures", "v", Some("s")),
                        property("x", "y", None),
                    ],
                }),
                skin: SkinPatch {
                    texture: Some(rl!("minecraft:skin").to_arc()),
                    cape: Some(rl!("minecraft:cape").to_arc()),
                    elytra: Some(rl!("minecraft:elytra").to_arc()),
                    model: Some(PlayerModelType::Slim),
                },
            },
            Profile {
                profile: ProfileIdentity::Partial {
                    name: None,
                    id: Some(ints_uuid(IntArray([1, 2, 3, 4]))),
                    properties: Vec::new(),
                },
                skin: SkinPatch::default(),
            },
            Profile {
                profile: ProfileIdentity::Partial {
                    name: Some(PlayerName::new("Steve").unwrap()),
                    id: None,
                    properties: vec![
                        property("textures", "v1", None),
                        property("textures", "v2", None),
                    ],
                },
                skin: SkinPatch {
                    model: Some(PlayerModelType::Wide),
                    ..Default::default()
                },
            },
        ]
    }
}

#[derive(Clone)]
struct SampleRegistry {
    names: Vec<ResourceLocation>,
    tags: Vec<(ResourceLocation, Vec<u16>)>,
}

/// Names and tags of a few entries of each registry the component samples name, keyed by the
/// type a value names the registry with.
#[derive(Default)]
pub struct SampleRegistries(HashMap<TypeId, SampleRegistry>);

impl SampleRegistries {
    fn with<R: 'static>(
        mut self,
        key: RegistryKey<R>,
        names: Vec<ResourceLocation>,
        tags: &[(&str, &[&str])],
    ) -> Self {
        let number = |member: &str| {
            names
                .iter()
                .position(|name| name.as_str() == format!("minecraft:{member}"))
                .and_then(|at| u16::try_from(at).ok())
                .unwrap_or_else(|| panic!("the sample {key} tags name {member}"))
        };
        let tags = tags
            .iter()
            .map(|(tag, members)| {
                let name = ResourceLocation::minecraft(tag)
                    .unwrap_or_else(|error| panic!("the sample tag {tag}: {error}"));
                (name, members.iter().map(|member| number(member)).collect())
            })
            .collect();
        let registry = SampleRegistry { names, tags };
        self.0.insert(TypeId::of::<R>(), registry);
        self
    }

    fn registry<R: 'static>(&self) -> Result<&SampleRegistry, String> {
        self.0
            .get(&TypeId::of::<R>())
            .ok_or_else(|| format!("the samples hold no {}", std::any::type_name::<R>()))
    }
}

impl Resolver for SampleRegistries {
    fn id<R: Registered>(
        &self,
        key: &ResourceKey<R>,
    ) -> Result<Id<R>, mcrs_minecraft_registries::ResolveError> {
        let registry = self.registry::<R>()?;
        registry
            .names
            .iter()
            .position(|name| name == key.location())
            .and_then(|at| Id::checked(u16::try_from(at).ok()?, registry.names.len()))
            .ok_or_else(|| format!("the samples hold no {}", key.location()).into())
    }

    fn tag<R: Registered>(
        &self,
        tag: &TagKey<R>,
    ) -> Result<TagId<R>, mcrs_minecraft_registries::ResolveError> {
        let registry = self.registry::<R>()?;
        registry
            .tags
            .iter()
            .position(|(name, _)| name.as_str() == tag.as_str())
            .and_then(|at| TagId::checked(u16::try_from(at).ok()?, registry.tags.len()))
            .ok_or_else(|| format!("the samples hold no tag #{}", tag.location()).into())
    }
}

impl Namer for SampleRegistries {
    fn name<R: Registered>(&self, id: Id<R>) -> Option<&ResourceLocation> {
        self.registry::<R>().ok()?.names.get(id.index())
    }

    fn tag_name<R: Registered>(&self, tag: TagId<R>) -> Option<&ResourceLocation> {
        let (name, _) = self.registry::<R>().ok()?.tags.get(tag.index())?;
        Some(name)
    }
}

/// A static registry numbers its entries as the generated constants do, so a constant names the
/// same entry in the samples as in the game; the listed names are what a peer numbers on the wire.
macro_rules! sample_numbering {
    ($entries:path; $($name:literal),+) => {
        $entries.iter().map(|&name| name.into()).collect::<Vec<ResourceLocation>>()
    };
    (; $($name:literal),+) => {
        vec![$(ResourceLocation::minecraft($name).unwrap()),+]
    };
}

macro_rules! sample_registries_table {
    ($($key:expr => $path:literal $(numbered $entries:path)? [$($name:literal),+] [$(($tag:literal => [$($member:literal),*])),*];)*) => {
        pub const SAMPLE_NAMES: &[(&str, &[&str])] = &[$(($path, &[$($name),+])),*];

        fn build_listed_registries(registries: SampleRegistries) -> SampleRegistries {
            $(
                let tags: &[(&str, &[&str])] = &[$(($tag, &[$($member),*])),*];
                let names = sample_numbering!($($entries)?; $($name),+);
                let registries = registries.with($key, names, tags);
            )*
            registries
        }
    };
}

sample_registries_table! {
    crate::keys::ITEM => "item" numbered crate::keys::Item::ENTRIES ["air", "stone", "diamond_sword", "apple", "bundle", "diamond"]
        [("planks" => ["stone"]), ("swords" => ["diamond_sword"])];
    crate::keys::MOB_EFFECT => "mob_effect" numbered crate::keys::MobEffect::ENTRIES ["speed", "slowness", "haste"] [];
    crate::keys::ENCHANTMENT => "enchantment" ["sharpness", "unbreaking"] [];
    crate::keys::DAMAGE_TYPE => "damage_type" ["in_fire", "lava"]
        [("is_fire" => ["in_fire", "lava"]), ("bypasses_shield" => ["lava"])];
    mcrs_minecraft_block::keys::BLOCK => "block" ["stone", "dirt"]
        [("mineable/pickaxe" => ["stone"]), ("logs" => ["dirt"])];
    mcrs_minecraft_entity::keys::ENTITY_TYPE => "entity_type" ["zombie", "pig", "skeleton", "player"]
        [("skeletons" => ["skeleton"])];
    mcrs_minecraft_block::keys::BLOCK_ENTITY_TYPE => "block_entity_type" ["chest", "sign"] [];
    crate::keys::POTION => "potion" numbered crate::keys::Potion::ENTRIES ["water", "swiftness", "healing"] [];
    mcrs_minecraft_entity::keys::ATTRIBUTE => "attribute"
        numbered mcrs_minecraft_entity::keys::Attribute::ENTRIES ["armor", "attack_damage"] [];
    crate::keys::BANNER_PATTERN => "banner_pattern" ["globe", "creeper"]
        [("pattern_item/globe" => ["globe"])];
    crate::keys::BLOCK_TRANSFORMER => "block_transformer" ["axe", "shovel"] [];
    mcrs_minecraft_entity::keys::VILLAGER_TYPE => "villager_type" ["plains", "desert"] [];
    mcrs_minecraft_entity::keys::WOLF_VARIANT => "wolf_variant" ["pale", "ashen"] [];
    mcrs_minecraft_entity::keys::WOLF_SOUND_VARIANT => "wolf_sound_variant" ["classic", "big"] [];
    mcrs_minecraft_entity::keys::PIG_VARIANT => "pig_variant" ["temperate", "cold"] [];
    mcrs_minecraft_entity::keys::PIG_SOUND_VARIANT => "pig_sound_variant" ["classic", "mini"] [];
    mcrs_minecraft_entity::keys::COW_VARIANT => "cow_variant" ["temperate", "warm"] [];
    mcrs_minecraft_entity::keys::COW_SOUND_VARIANT => "cow_sound_variant" ["classic", "moody"] [];
    mcrs_minecraft_entity::keys::CHICKEN_VARIANT => "chicken_variant" ["temperate", "cold"] [];
    mcrs_minecraft_entity::keys::CHICKEN_SOUND_VARIANT => "chicken_sound_variant" ["classic", "picky"] [];
    mcrs_minecraft_entity::keys::ZOMBIE_NAUTILUS_VARIANT => "zombie_nautilus_variant" ["temperate", "warm"] [];
    mcrs_minecraft_entity::keys::FROG_VARIANT => "frog_variant" ["temperate", "warm"] [];
    mcrs_minecraft_entity::keys::CAT_VARIANT => "cat_variant" ["tabby", "jellie"] [];
    mcrs_minecraft_entity::keys::CAT_SOUND_VARIANT => "cat_sound_variant" ["classic", "royal"] [];
    crate::keys::DECORATED_POT_PATTERN => "decorated_pot_pattern" ["angler", "skull"] [];
    crate::keys::TRIM_MATERIAL => "trim_material" ["amethyst", "gold"] [];
    crate::keys::TRIM_PATTERN => "trim_pattern" ["coast", "sentry", "vex"] [];
    crate::keys::INSTRUMENT => "instrument" ["ponder_goat_horn"] [];
    crate::keys::JUKEBOX_SONG => "jukebox_song" ["pigstep", "cat"] [];
    crate::keys::PAINTING_VARIANT => "painting_variant" ["kebab"] [];
}

fn build_sample_registries() -> SampleRegistries {
    let sounds = mcrs_minecraft_sound::keys::sound_event::ENTRIES
        .iter()
        .map(|&name| name.into())
        .collect();
    build_listed_registries(SampleRegistries::default()).with(
        mcrs_minecraft_sound::keys::SOUND_EVENT,
        sounds,
        &[],
    )
}

pub fn sample_registries() -> &'static SampleRegistries {
    static SET: LazyLock<SampleRegistries> = LazyLock::new(build_sample_registries);
    &SET
}

fn sample_key<R>(path: &str) -> ResourceKey<R> {
    ResourceKey::from_location(
        ResourceLocation::minecraft(path)
            .unwrap_or_else(|error| panic!("the sample name {path}: {error}")),
    )
}

pub fn entry<R: Registered>(path: &str) -> Id<R> {
    sample_registries()
        .id(&sample_key(path))
        .unwrap_or_else(|error| panic!("{error}"))
}

pub fn tag_set<R: Registered>(path: &str) -> IdSet<R> {
    sample_registries()
        .tag(&TagKey::from_location(
            sample_key::<R>(path).location().clone(),
        ))
        .map(IdSet::Tag)
        .unwrap_or_else(|error| panic!("{error}"))
}

pub fn one_set<R: Registered>(path: &str) -> IdSet<R> {
    list_set(&[path])
}

pub fn list_set<R: Registered>(paths: &[&str]) -> IdSet<R> {
    IdSet::List(paths.iter().map(|path| entry(path)).collect())
}
