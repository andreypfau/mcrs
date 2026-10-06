pub trait Sample: Sized {
    fn samples() -> Vec<Self>;

    /// The NBT tag id each dotted path below this sample's persistent form
    /// must carry, `""` naming the root; a width vanilla would write
    /// differently is a persistence bug the round trip alone cannot see.
    fn nbt_tags(&self) -> Vec<(&'static str, u8)> {
        Vec::new()
    }
}

use std::sync::{Arc, LazyLock};

use mcrs_minecraft_core::codec::IntArray;
use mcrs_minecraft_core::{RegistryKey, ResourceLocation, rl};
use mcrs_minecraft_keys as keys;
use mcrs_minecraft_nbt::{COMPOUND_ID, INT_ARRAY_ID, LIST_ID, STRING_ID};
use mcrs_minecraft_profile::{
    GameProfileValue, PlayerModelType, PlayerName, Profile, ProfileIdentity, Property, SkinPatch,
    ints_uuid,
};
use mcrs_minecraft_registry::tags::TagSource;
use mcrs_minecraft_registry::{Registry, RegistrySet, StaticRegistry, TagRules, build_tags};

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

type Name = ResourceLocation;

fn registry_with_tags<R: 'static>(
    set: RegistrySet,
    key: RegistryKey<R>,
    names: &[&str],
    tags: &[(&str, &[&str])],
) -> RegistrySet {
    let name = |path: &str| -> Name { ResourceLocation::minecraft(path).unwrap() };
    let registry = Registry::new(key, names.iter().map(|path| name(path)))
        .unwrap_or_else(|error| panic!("the sample {key} registry: {error}"));
    let files: Vec<(Name, String)> = tags
        .iter()
        .map(|(tag, members)| {
            let values: Vec<String> = members
                .iter()
                .map(|member| format!("minecraft:{member}"))
                .collect();
            (
                name(tag),
                serde_json::json!({ "values": values }).to_string(),
            )
        })
        .collect();
    let sources: Vec<(Name, Vec<TagSource<'_>>)> = files
        .iter()
        .map(|(tag, json)| {
            let source = TagSource {
                pack: "sample",
                path: "sample.json",
                bytes: json.as_bytes(),
            };
            (tag.clone(), vec![source])
        })
        .collect();
    let (table, problems) = build_tags(registry.table(), TagRules::World, &sources, None);
    assert!(problems.is_empty(), "the sample {key} tags: {problems:?}");
    set.with(registry)
        .unwrap_or_else(|error| panic!("the sample {key} registry: {error}"))
        .with_tags(Arc::new(table))
}

macro_rules! sample_registries_table {
    ($($key:expr => $path:literal [$($name:literal),+] [$(($tag:literal => [$($member:literal),*])),*];)*) => {
        pub const SAMPLE_NAMES: &[(&str, &[&str])] = &[$(($path, &[$($name),+])),*];

        fn build_listed_registries(set: RegistrySet) -> RegistrySet {
            $(
                let tags: &[(&str, &[&str])] = &[$(($tag, &[$($member),*])),*];
                let set = registry_with_tags(set, $key, &[$($name),+], tags);
            )*
            set
        }
    };
}

sample_registries_table! {
    keys::ITEM => "item" ["air", "stone", "diamond_sword", "apple", "bundle", "diamond"]
        [("planks" => ["stone"]), ("swords" => ["diamond_sword"])];
    keys::MOB_EFFECT => "mob_effect" ["speed", "slowness", "haste"] [];
    keys::ENCHANTMENT => "enchantment" ["sharpness", "unbreaking"] [];
    keys::DAMAGE_TYPE => "damage_type" ["in_fire", "lava"]
        [("is_fire" => ["in_fire", "lava"]), ("bypasses_shield" => ["lava"])];
    keys::BLOCK => "block" ["stone", "dirt"]
        [("mineable/pickaxe" => ["stone"]), ("logs" => ["dirt"])];
    keys::ENTITY_TYPE => "entity_type" ["zombie", "pig", "skeleton", "player"]
        [("skeletons" => ["skeleton"])];
    keys::BLOCK_ENTITY_TYPE => "block_entity_type" ["chest", "sign"] [];
    keys::POTION => "potion" ["water", "swiftness", "healing"] [];
    keys::ATTRIBUTE => "attribute" ["armor", "attack_damage"] [];
    keys::BANNER_PATTERN => "banner_pattern" ["globe", "creeper"]
        [("pattern_item/globe" => ["globe"])];
    keys::BLOCK_TRANSFORMER => "block_transformer" ["axe", "shovel"] [];
    keys::VILLAGER_TYPE => "villager_type" ["plains", "desert"] [];
    keys::WOLF_VARIANT => "wolf_variant" ["pale", "ashen"] [];
    keys::WOLF_SOUND_VARIANT => "wolf_sound_variant" ["classic", "big"] [];
    keys::PIG_VARIANT => "pig_variant" ["temperate", "cold"] [];
    keys::PIG_SOUND_VARIANT => "pig_sound_variant" ["classic", "mini"] [];
    keys::COW_VARIANT => "cow_variant" ["temperate", "warm"] [];
    keys::COW_SOUND_VARIANT => "cow_sound_variant" ["classic", "moody"] [];
    keys::CHICKEN_VARIANT => "chicken_variant" ["temperate", "cold"] [];
    keys::CHICKEN_SOUND_VARIANT => "chicken_sound_variant" ["classic", "picky"] [];
    keys::ZOMBIE_NAUTILUS_VARIANT => "zombie_nautilus_variant" ["temperate", "warm"] [];
    keys::FROG_VARIANT => "frog_variant" ["temperate", "warm"] [];
    keys::CAT_VARIANT => "cat_variant" ["tabby", "jellie"] [];
    keys::CAT_SOUND_VARIANT => "cat_sound_variant" ["classic", "royal"] [];
    keys::DECORATED_POT_PATTERN => "decorated_pot_pattern" ["angler", "skull"] [];
    keys::TRIM_MATERIAL => "trim_material" ["amethyst", "gold"] [];
    keys::TRIM_PATTERN => "trim_pattern" ["coast", "sentry", "vex"] [];
    keys::INSTRUMENT => "instrument" ["ponder_goat_horn"] [];
    keys::JUKEBOX_SONG => "jukebox_song" ["pigstep", "cat"] [];
    keys::PAINTING_VARIANT => "painting_variant" ["kebab"] [];
}

/// A static registry numbers its entries as the generated constants do, so a
/// constant names the same entry in the samples as in the game.
fn build_sample_registries() -> RegistrySet {
    let sounds = Registry::<keys::SoundEvent>::new(
        keys::SOUND_EVENT,
        keys::SoundEvent::NAMES
            .iter()
            .map(|name| ResourceLocation::read(name).expect("a generated name parses")),
    )
    .unwrap_or_else(|error| panic!("the sample sound_event registry: {error}"));
    build_listed_registries(RegistrySet::new())
        .with(sounds)
        .unwrap_or_else(|error| panic!("the sample sound_event registry: {error}"))
}

pub fn sample_registries() -> &'static RegistrySet {
    static SET: LazyLock<RegistrySet> = LazyLock::new(build_sample_registries);
    &SET
}
