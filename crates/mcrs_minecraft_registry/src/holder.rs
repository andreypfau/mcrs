use std::fmt;
use std::marker::PhantomData;

use mcrs_minecraft_core::{ResourceKey, ResourceLocation};
use serde::de::{DeserializeOwned, MapAccess, Visitor, value};
use serde::ser::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

pub trait RegistryName: Clone + PartialEq + fmt::Debug {
    const NAME: &'static str;
}

macro_rules! registries {
    ($($marker:ident = $name:literal),* $(,)?) => {$(
        #[derive(Clone, Copy, PartialEq, Eq, Debug)]
        pub enum $marker {}
        impl RegistryName for $marker {
            const NAME: &'static str = $name;
        }
    )*};
}

registries! {
    ItemReg = "item",
    BlockReg = "block",
    EntityTypeReg = "entity_type",
    BlockEntityTypeReg = "block_entity_type",
    MobEffectReg = "mob_effect",
    PotionReg = "potion",
    AttributeReg = "attribute",
    EnchantmentReg = "enchantment",
    DamageTypeReg = "damage_type",
    SoundEventReg = "sound_event",
    BlockTransformerReg = "block_transformer",
    BannerPatternReg = "banner_pattern",
    DecoratedPotPatternReg = "decorated_pot_pattern",
    InstrumentReg = "instrument",
    JukeboxSongReg = "jukebox_song",
    TrimMaterialReg = "trim_material",
    TrimPatternReg = "trim_pattern",
    PaintingVariantReg = "painting_variant",
    VillagerTypeReg = "villager_type",
    WolfVariantReg = "wolf_variant",
    WolfSoundVariantReg = "wolf_sound_variant",
    PigVariantReg = "pig_variant",
    PigSoundVariantReg = "pig_sound_variant",
    CowVariantReg = "cow_variant",
    CowSoundVariantReg = "cow_sound_variant",
    ChickenVariantReg = "chicken_variant",
    ChickenSoundVariantReg = "chicken_sound_variant",
    ZombieNautilusVariantReg = "zombie_nautilus_variant",
    FrogVariantReg = "frog_variant",
    CatVariantReg = "cat_variant",
    CatSoundVariantReg = "cat_sound_variant",
    DimensionReg = "dimension",
    LootTableReg = "loot_table",
    RecipeReg = "recipe",
    MapDecorationTypeReg = "map_decoration_type",
    ContextIntProviderReg = "context_int_provider",
    ContextFloatProviderReg = "context_float_provider",
    DialogReg = "dialog",
}

pub trait Registered: Serialize + DeserializeOwned + Clone + PartialEq + fmt::Debug {
    type Registry: RegistryName;
}

/// A registry id, or the entry itself written inline.
#[derive(Clone, PartialEq, Debug)]
pub enum Holder<T: Registered> {
    Reference(ResourceKey<T::Registry>),
    Direct(T),
}

impl<T: Registered> Holder<T> {
    pub fn reference(location: ResourceLocation) -> Self {
        Holder::Reference(ResourceKey::from_location(location))
    }
}

impl<T: Registered> Serialize for Holder<T> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            Holder::Reference(key) => key.serialize(s),
            Holder::Direct(value) => value.serialize(s),
        }
    }
}

impl<'de, T: Registered> Deserialize<'de> for Holder<T> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct HolderVisitor<T>(PhantomData<T>);

        impl<'de, T: Registered> Visitor<'de> for HolderVisitor<T> {
            type Value = Holder<T>;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                write!(f, "a {} id or an inline entry", T::Registry::NAME)
            }

            fn visit_str<E: serde::de::Error>(self, text: &str) -> Result<Self::Value, E> {
                ResourceLocation::read(text)
                    .map(Holder::reference)
                    .map_err(E::custom)
            }

            fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<Self::Value, A::Error> {
                T::deserialize(value::MapAccessDeserializer::new(map)).map(Holder::Direct)
            }
        }

        d.deserialize_any(HolderVisitor(PhantomData))
    }
}

/// A holder whose persistent form is the registry id only; the inline entry
/// exists on the wire alone.
#[derive(Clone, PartialEq, Debug)]
pub struct HolderWireOnly<T: Registered>(pub Holder<T>);

impl<T: Registered> Serialize for HolderWireOnly<T> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match &self.0 {
            Holder::Reference(key) => key.serialize(s),
            Holder::Direct(_) => Err(S::Error::custom(format_args!(
                "an inline {} entry has no persistent form",
                T::Registry::NAME
            ))),
        }
    }
}

impl<'de, T: Registered> Deserialize<'de> for HolderWireOnly<T> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        ResourceKey::deserialize(d).map(|key| HolderWireOnly(Holder::Reference(key)))
    }
}
