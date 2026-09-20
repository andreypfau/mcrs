use std::cmp::Ordering;
use std::fmt;
use std::io::Write;
use std::marker::PhantomData;

use anyhow::ensure;
use mcrs_minecraft_core::codec::{default_true, float_value, int_value};
use mcrs_minecraft_core::{ResourceKey, ResourceLocation};
use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_nbt::tag::NbtTag;
use mcrs_minecraft_nbt::{from_tag, nbt_flag, nbt_int_array};
use mcrs_minecraft_registry::RegistryLookup;
use serde::de::{DeserializeOwned, Error as _, IgnoredAny, MapAccess, SeqAccess, Visitor, value};
use serde::ser::{Error as _, SerializeMap};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::item::ctx::{DecodeCtx, EncodeCtx, ctx_free};
use crate::{Bounded, Decode, Encode, VarInt};

pub use crate::text::optional_flag;

pub trait RegistryName {
    const NAME: &'static str;
}

macro_rules! registries {
    ($($marker:ident = $name:literal),* $(,)?) => {$(
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
    ConsumeEffectTypeReg = "consume_effect_type",
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
    DataComponentTypeReg = "data_component_type",
    DataComponentPredicateTypeReg = "data_component_predicate_type",
    DimensionReg = "dimension",
    LootTableReg = "loot_table",
    RecipeReg = "recipe",
    MapDecorationTypeReg = "map_decoration_type",
    ContextIntProviderReg = "context_int_provider",
    ContextFloatProviderReg = "context_float_provider",
    DialogReg = "dialog",
}

/// A value that lives in a registry and may also be written inline.
pub trait Registered:
    EncodeCtx + for<'a> DecodeCtx<'a> + Serialize + DeserializeOwned + Clone + PartialEq + fmt::Debug
{
    type Registry: RegistryName;
}

/// `RegistryCodecs.holder`: a registry id, or the entry itself written inline.
pub enum Holder<T: Registered> {
    Reference(ResourceKey<T::Registry>),
    Direct(T),
}

impl<T: Registered> Clone for Holder<T> {
    fn clone(&self) -> Self {
        match self {
            Holder::Reference(key) => Holder::Reference(key.clone()),
            Holder::Direct(value) => Holder::Direct(value.clone()),
        }
    }
}

impl<T: Registered> PartialEq for Holder<T> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Holder::Reference(a), Holder::Reference(b)) => a == b,
            (Holder::Direct(a), Holder::Direct(b)) => a == b,
            _ => false,
        }
    }
}

impl<T: Registered> fmt::Debug for Holder<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Holder::Reference(key) => f.debug_tuple("Reference").field(key).finish(),
            Holder::Direct(value) => f.debug_tuple("Direct").field(value).finish(),
        }
    }
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
pub struct HolderWireOnly<T: Registered>(pub Holder<T>);

impl<T: Registered> Clone for HolderWireOnly<T> {
    fn clone(&self) -> Self {
        HolderWireOnly(self.0.clone())
    }
}

impl<T: Registered> PartialEq for HolderWireOnly<T> {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl<T: Registered> fmt::Debug for HolderWireOnly<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("HolderWireOnly").field(&self.0).finish()
    }
}

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

impl<T: Registered> EncodeCtx for HolderWireOnly<T> {
    fn encode_ctx(&self, ctx: &dyn RegistryLookup, w: impl Write) -> anyhow::Result<()> {
        self.0.encode_ctx(ctx, w)
    }
}

impl<'a, T: Registered> DecodeCtx<'a> for HolderWireOnly<T> {
    fn decode_ctx(ctx: &dyn RegistryLookup, r: &mut &'a [u8]) -> anyhow::Result<Self> {
        Holder::decode_ctx(ctx, r).map(HolderWireOnly)
    }
}

/// `Filterable.codec`: `{raw, filtered?}`, read leniently from a bare value.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Filterable<T> {
    pub raw: T,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filtered: Option<T>,
}

impl<T> Filterable<T> {
    pub fn pass_through(raw: T) -> Self {
        Filterable {
            raw,
            filtered: None,
        }
    }
}

impl<'de, T: DeserializeOwned> Deserialize<'de> for Filterable<T> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields, bound = "T: DeserializeOwned")]
        struct Repr<T> {
            raw: T,
            #[serde(default)]
            filtered: Option<T>,
        }

        struct FilterableVisitor<T>(PhantomData<T>);

        impl<'de, T: DeserializeOwned> Visitor<'de> for FilterableVisitor<T> {
            type Value = Filterable<T>;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a filterable value")
            }

            fn visit_str<E: serde::de::Error>(self, text: &str) -> Result<Self::Value, E> {
                T::deserialize(value::StrDeserializer::new(text)).map(Filterable::pass_through)
            }

            fn visit_seq<A: SeqAccess<'de>>(self, seq: A) -> Result<Self::Value, A::Error> {
                T::deserialize(value::SeqAccessDeserializer::new(seq)).map(Filterable::pass_through)
            }

            fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<Self::Value, A::Error> {
                let compound = NbtCompound::deserialize(value::MapAccessDeserializer::new(map))?;
                let tag = NbtTag::Compound(compound);
                if tag
                    .extract_compound()
                    .is_some_and(|c| c.get("raw").is_some())
                {
                    let Repr { raw, filtered } = from_tag(tag).map_err(A::Error::custom)?;
                    Ok(Filterable { raw, filtered })
                } else {
                    from_tag(tag)
                        .map(Filterable::pass_through)
                        .map_err(A::Error::custom)
                }
            }
        }

        d.deserialize_any(FilterableVisitor(PhantomData))
    }
}

impl<T: EncodeCtx> EncodeCtx for Filterable<T> {
    fn encode_ctx(&self, ctx: &dyn RegistryLookup, mut w: impl Write) -> anyhow::Result<()> {
        self.raw.encode_ctx(ctx, &mut w)?;
        self.filtered.encode_ctx(ctx, w)
    }
}

impl<'a, T: DecodeCtx<'a>> DecodeCtx<'a> for Filterable<T> {
    fn decode_ctx(ctx: &dyn RegistryLookup, r: &mut &'a [u8]) -> anyhow::Result<Self> {
        Ok(Filterable {
            raw: T::decode_ctx(ctx, r)?,
            filtered: Option::decode_ctx(ctx, r)?,
        })
    }
}

macro_rules! resolvable {
    ($name:ident, $scalar:ty, $registry:ident, $expecting:literal, $number:ident) => {
        #[derive(Clone, Debug, PartialEq)]
        pub enum $name {
            Constant($scalar),
            Reference(ResourceKey<$registry>),
        }

        impl Serialize for $name {
            fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                match self {
                    Self::Constant(value) => value.serialize(s),
                    Self::Reference(key) => key.serialize(s),
                }
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                struct V(bool);

                impl Visitor<'_> for V {
                    type Value = $name;

                    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                        f.write_str($expecting)
                    }

                    fn visit_str<E: serde::de::Error>(self, text: &str) -> Result<$name, E> {
                        ResourceLocation::read(text)
                            .map(|location| $name::Reference(ResourceKey::from_location(location)))
                            .map_err(E::custom)
                    }

                    fn visit_i64<E: serde::de::Error>(self, value: i64) -> Result<$name, E> {
                        $number(Number::I64(value, self.0))
                            .map($name::Constant)
                            .map_err(E::custom)
                    }

                    fn visit_u64<E: serde::de::Error>(self, value: u64) -> Result<$name, E> {
                        $number(Number::U64(value, self.0))
                            .map($name::Constant)
                            .map_err(E::custom)
                    }

                    fn visit_f64<E: serde::de::Error>(self, value: f64) -> Result<$name, E> {
                        $number(Number::F64(value, self.0))
                            .map($name::Constant)
                            .map_err(E::custom)
                    }
                }

                let human_readable = d.is_human_readable();
                d.deserialize_any(V(human_readable))
            }
        }

        impl EncodeCtx for $name {
            fn encode_ctx(&self, _: &dyn RegistryLookup, mut w: impl Write) -> anyhow::Result<()> {
                match self {
                    Self::Constant(value) => {
                        true.encode(&mut w)?;
                        value.encode(w)
                    }
                    Self::Reference(key) => {
                        false.encode(&mut w)?;
                        key.location().encode(w)
                    }
                }
            }
        }

        impl DecodeCtx<'_> for $name {
            fn decode_ctx(_: &dyn RegistryLookup, r: &mut &[u8]) -> anyhow::Result<Self> {
                Ok(match bool::decode(r)? {
                    true => Self::Constant(Decode::decode(r)?),
                    false => {
                        Self::Reference(ResourceKey::from_location(ResourceLocation::decode(r)?))
                    }
                })
            }
        }
    };
}

resolvable!(
    ResolvableInt,
    i32,
    ContextIntProviderReg,
    "an int or a context int provider id",
    int_value
);
resolvable!(
    ResolvableFloat,
    f32,
    ContextFloatProviderReg,
    "a float or a context float provider id",
    float_value
);

/// A number a visitor already holds, replayed through the `Codec.INT` /
/// `Codec.FLOAT` readers; the flag is the source's `is_human_readable`.
enum Number {
    I64(i64, bool),
    U64(u64, bool),
    F64(f64, bool),
}

impl<'de> Deserializer<'de> for Number {
    type Error = value::Error;

    fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        match self {
            Number::I64(v, _) => visitor.visit_i64(v),
            Number::U64(v, _) => visitor.visit_u64(v),
            Number::F64(v, _) => visitor.visit_f64(v),
        }
    }

    fn is_human_readable(&self) -> bool {
        match *self {
            Number::I64(_, h) | Number::U64(_, h) | Number::F64(_, h) => h,
        }
    }

    serde::forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string bytes byte_buf
        option unit unit_struct newtype_struct seq tuple tuple_struct map struct enum
        identifier ignored_any
    }
}

/// `ExtraCodecs.UNSIGNED_BYTE`: any number's `intValue()`, low byte kept.
pub fn unsigned_byte<'de, D: Deserializer<'de>>(d: D) -> Result<i8, D::Error> {
    int_value(d).map(|v| v as i8)
}

/// `lenientOptionalFieldOf`: a present-but-malformed value reads as absent,
/// and so does a JSON `null`, which `JsonOps` reports as no entry at all.
/// The value is buffered first so a failed parse never leaves a streaming
/// input half-consumed.
pub fn lenient<'de, D: Deserializer<'de>, T: DeserializeOwned + Default>(
    d: D,
) -> Result<T, D::Error> {
    let tag = <Option<NbtTag> as Deserialize>::deserialize(d)?;
    Ok(tag.and_then(|tag| from_tag(tag).ok()).unwrap_or_default())
}

/// `lenientOptionalFieldOf(Codec.FLOAT)`: only a number reads as a value;
/// anything else present (a JSON boolean or null included) is consumed and
/// reads as absent.
pub fn lenient_float<'de, D: Deserializer<'de>>(d: D) -> Result<Option<f32>, D::Error> {
    struct LenientFloat;

    impl<'de> Visitor<'de> for LenientFloat {
        type Value = Option<f32>;

        fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
            f.write_str("any value")
        }

        fn visit_i64<E: serde::de::Error>(self, v: i64) -> Result<Self::Value, E> {
            Ok(Some(v as f32))
        }

        fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<Self::Value, E> {
            Ok(Some(v as f32))
        }

        fn visit_f64<E: serde::de::Error>(self, v: f64) -> Result<Self::Value, E> {
            Ok(Some(v as f32))
        }

        fn visit_bool<E: serde::de::Error>(self, _: bool) -> Result<Self::Value, E> {
            Ok(None)
        }

        fn visit_str<E: serde::de::Error>(self, _: &str) -> Result<Self::Value, E> {
            Ok(None)
        }

        fn visit_bytes<E: serde::de::Error>(self, _: &[u8]) -> Result<Self::Value, E> {
            Ok(None)
        }

        fn visit_unit<E: serde::de::Error>(self) -> Result<Self::Value, E> {
            Ok(None)
        }

        fn visit_none<E: serde::de::Error>(self) -> Result<Self::Value, E> {
            Ok(None)
        }

        fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
            while seq.next_element::<IgnoredAny>()?.is_some() {}
            Ok(None)
        }

        fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
            while map.next_entry::<IgnoredAny, IgnoredAny>()?.is_some() {}
            Ok(None)
        }
    }

    d.deserialize_any(LenientFloat)
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MobEffectInstance {
    pub id: ResourceKey<MobEffectReg>,
    #[serde(flatten)]
    pub details: MobEffectDetails,
}

/// `show_icon` is resolved on read (absent means `show_particles`) and always
/// written, as `MobEffectInstance.Details.MAP_CODEC` does.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(from = "MobEffectDetailsRepr")]
pub struct MobEffectDetails {
    #[serde(skip_serializing_if = "is_zero_i8")]
    pub amplifier: i8,
    #[serde(skip_serializing_if = "is_zero_i32")]
    pub duration: i32,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub ambient: bool,
    #[serde(skip_serializing_if = "std::clone::Clone::clone")]
    pub show_particles: bool,
    pub show_icon: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hidden_effect: Option<Box<MobEffectDetails>>,
}

#[derive(Deserialize)]
struct MobEffectDetailsRepr {
    #[serde(default, deserialize_with = "unsigned_byte")]
    amplifier: i8,
    #[serde(default, deserialize_with = "int_value")]
    duration: i32,
    #[serde(default, deserialize_with = "nbt_flag")]
    ambient: bool,
    #[serde(default = "default_true", deserialize_with = "nbt_flag")]
    show_particles: bool,
    #[serde(default, deserialize_with = "optional_flag")]
    show_icon: Option<bool>,
    #[serde(default)]
    hidden_effect: Option<Box<MobEffectDetails>>,
}

impl From<MobEffectDetailsRepr> for MobEffectDetails {
    fn from(repr: MobEffectDetailsRepr) -> Self {
        MobEffectDetails {
            amplifier: repr.amplifier,
            duration: repr.duration,
            ambient: repr.ambient,
            show_particles: repr.show_particles,
            show_icon: repr.show_icon.unwrap_or(repr.show_particles),
            hidden_effect: repr.hidden_effect,
        }
    }
}

impl Default for MobEffectDetails {
    fn default() -> Self {
        MobEffectDetails {
            amplifier: 0,
            duration: 0,
            ambient: false,
            show_particles: true,
            show_icon: true,
            hidden_effect: None,
        }
    }
}

impl MobEffectDetails {
    pub fn amplifier(&self) -> u8 {
        self.amplifier as u8
    }
}

fn is_zero_i8(value: &i8) -> bool {
    *value == 0
}

fn is_zero_i32(value: &i32) -> bool {
    *value == 0
}

impl EncodeCtx for MobEffectInstance {
    fn encode_ctx(&self, ctx: &dyn RegistryLookup, mut w: impl Write) -> anyhow::Result<()> {
        self.id.encode_ctx(ctx, &mut w)?;
        self.details.encode_ctx(ctx, w)
    }
}

impl DecodeCtx<'_> for MobEffectInstance {
    fn decode_ctx(ctx: &dyn RegistryLookup, r: &mut &[u8]) -> anyhow::Result<Self> {
        Ok(MobEffectInstance {
            id: ResourceKey::decode_ctx(ctx, r)?,
            details: MobEffectDetails::decode_ctx(ctx, r)?,
        })
    }
}

impl EncodeCtx for MobEffectDetails {
    fn encode_ctx(&self, ctx: &dyn RegistryLookup, mut w: impl Write) -> anyhow::Result<()> {
        VarInt(self.amplifier() as i32).encode(&mut w)?;
        VarInt(self.duration).encode(&mut w)?;
        self.ambient.encode(&mut w)?;
        self.show_particles.encode(&mut w)?;
        self.show_icon.encode(&mut w)?;
        match &self.hidden_effect {
            Some(hidden) => {
                true.encode(&mut w)?;
                hidden.encode_ctx(ctx, w)
            }
            None => false.encode(w),
        }
    }
}

impl DecodeCtx<'_> for MobEffectDetails {
    #[allow(clippy::only_used_in_recursion)]
    fn decode_ctx(ctx: &dyn RegistryLookup, r: &mut &[u8]) -> anyhow::Result<Self> {
        let amplifier = VarInt::decode(r)?.0.clamp(0, 255);
        let duration = VarInt::decode(r)?.0;
        let ambient = bool::decode(r)?;
        let show_particles = bool::decode(r)?;
        let show_icon = bool::decode(r)?;
        let hidden_effect = match bool::decode(r)? {
            true => Some(Box::new(MobEffectDetails::decode_ctx(ctx, r)?)),
            false => None,
        };
        Ok(MobEffectDetails {
            amplifier: amplifier as i8,
            duration,
            ambient,
            show_particles,
            show_icon,
            hidden_effect,
        })
    }
}

/// `TypedEntityData.codec`: a compound whose `id` names the type, read from a
/// compound or an SNBT string; the `id` is lifted out and written back first.
#[derive(Clone, PartialEq)]
pub struct TypedEntityData<R> {
    pub id: ResourceKey<R>,
    pub tag: NbtCompound,
}

impl<R> fmt::Debug for TypedEntityData<R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TypedEntityData")
            .field("id", &self.id)
            .field("tag", &self.tag)
            .finish()
    }
}

impl<R> Serialize for TypedEntityData<R> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut map = s.serialize_map(Some(self.tag.child_tags.len() + 1))?;
        map.serialize_entry("id", &self.id)?;
        for (key, value) in &self.tag.child_tags {
            if key != "id" {
                map.serialize_entry(key, value)?;
            }
        }
        map.end()
    }
}

impl<'de, R> Deserialize<'de> for TypedEntityData<R> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let mut tag = compound_or_snbt(d)?;
        let Some(id) = tag.child_tags.iter().position(|(key, _)| key == "id") else {
            return Err(D::Error::custom("Expected 'id' field"));
        };
        let (_, id) = tag.child_tags.remove(id);
        let Some(id) = id.extract_string() else {
            return Err(D::Error::custom("Expected 'id' field to be a string"));
        };
        let id = ResourceLocation::read(id).map_err(D::Error::custom)?;
        Ok(TypedEntityData {
            id: ResourceKey::from_location(id),
            tag,
        })
    }
}

impl<R: RegistryName> EncodeCtx for TypedEntityData<R> {
    fn encode_ctx(&self, ctx: &dyn RegistryLookup, mut w: impl Write) -> anyhow::Result<()> {
        self.id.encode_ctx(ctx, &mut w)?;
        self.tag.encode(w)
    }
}

impl<R: RegistryName> DecodeCtx<'_> for TypedEntityData<R> {
    fn decode_ctx(ctx: &dyn RegistryLookup, r: &mut &[u8]) -> anyhow::Result<Self> {
        let id = ResourceKey::decode_ctx(ctx, r)?;
        ensure!(
            r.first() == Some(&mcrs_minecraft_nbt::COMPOUND_ID),
            "expected a compound tag"
        );
        Ok(TypedEntityData {
            id,
            tag: NbtCompound::decode(r)?,
        })
    }
}

/// `CustomData.COMPOUND_TAG_CODEC`: a compound, or on read an SNBT string
/// that parses to one.
pub fn compound_or_snbt<'de, D: Deserializer<'de>>(d: D) -> Result<NbtCompound, D::Error> {
    struct CompoundVisitor;

    impl<'de> Visitor<'de> for CompoundVisitor {
        type Value = NbtCompound;

        fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
            f.write_str("a compound or its SNBT text")
        }

        fn visit_str<E: serde::de::Error>(self, text: &str) -> Result<NbtCompound, E> {
            mcrs_minecraft_nbt::snbt::parse_compound(text).map_err(E::custom)
        }

        fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<NbtCompound, A::Error> {
            NbtCompound::deserialize(value::MapAccessDeserializer::new(map))
        }
    }

    d.deserialize_any(CompoundVisitor)
}

macro_rules! ordinal_enum {
    ($(#[$meta:meta])* $name:ident { $($variant:ident),* $(,)? }) => {
        $(#[$meta])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(rename_all = "snake_case")]
        pub enum $name {
            $($variant),*
        }

        impl $name {
            pub const ALL: &'static [Self] = &[$(Self::$variant),*];
        }

        impl Encode for $name {
            fn encode(&self, w: impl Write) -> anyhow::Result<()> {
                VarInt(*self as i32).encode(w)
            }
        }

        /// Out-of-range ids read as the first variant, `ByIdMap` `ZERO`.
        impl Decode<'_> for $name {
            fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
                let id = VarInt::decode(r)?.0;
                Ok(usize::try_from(id)
                    .ok()
                    .and_then(|id| Self::ALL.get(id))
                    .copied()
                    .unwrap_or(Self::ALL[0]))
            }
        }

        ctx_free!($name);
    };
}
#[allow(unused_imports)]
pub(crate) use ordinal_enum;

ordinal_enum! {
    EquipmentSlotGroup { Any, Mainhand, Offhand, Hand, Feet, Legs, Chest, Head, Armor, Body, Saddle }
}

ordinal_enum! {
    ItemUseAnimation { None, Eat, Drink, Block, Bow, Trident, Crossbow, Spyglass, TootHorn, Brush, Bundle, Spear }
}

/// `ARGB.color` over `as8BitChannel`: each channel floored to eight bits and
/// masked, so an out-of-range channel never bleeds into its neighbour.
fn argb_from_floats(a: f32, r: f32, g: f32, b: f32) -> i32 {
    let channel = |v: f32| (v * 255.0).floor() as i32 & 0xFF;
    (channel(a) << 24) | (channel(r) << 16) | (channel(g) << 8) | channel(b)
}

/// `Codec.INT`, or on read `N` float channels in 0..=1.
macro_rules! color_int {
    ($(#[$meta:meta])* $name:ident, $channels:literal, $from:expr) => {
        $(#[$meta])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default, Encode, Decode)]
        pub struct $name(pub i32);

        impl Serialize for $name {
            fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                s.serialize_i32(self.0)
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                struct ColorVisitor(bool);

                impl<'de> Visitor<'de> for ColorVisitor {
                    type Value = $name;

                    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                        write!(f, "an int or {} color channels", $channels)
                    }

                    fn visit_i64<E: serde::de::Error>(self, v: i64) -> Result<$name, E> {
                        int_value(Number::I64(v, self.0)).map($name).map_err(E::custom)
                    }

                    fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<$name, E> {
                        int_value(Number::U64(v, self.0)).map($name).map_err(E::custom)
                    }

                    fn visit_f64<E: serde::de::Error>(self, v: f64) -> Result<$name, E> {
                        int_value(Number::F64(v, self.0)).map($name).map_err(E::custom)
                    }

                    fn visit_seq<A: SeqAccess<'de>>(self, seq: A) -> Result<$name, A::Error> {
                        let channels: Vec<f32> =
                            Vec::deserialize(value::SeqAccessDeserializer::new(seq))?;
                        let channels: [f32; $channels] = channels.try_into().map_err(|c: Vec<f32>| {
                            A::Error::custom(format_args!(
                                "expected {} color channels, got {}",
                                $channels,
                                c.len()
                            ))
                        })?;
                        Ok($name($from(channels)))
                    }
                }

                let human_readable = d.is_human_readable();
                d.deserialize_any(ColorVisitor(human_readable))
            }
        }

        ctx_free!($name);
    };
}

color_int!(
    /// `ExtraCodecs.RGB_COLOR_CODEC`: an int, or on read `[r, g, b]`.
    RgbInt,
    3,
    |[r, g, b]: [f32; 3]| argb_from_floats(1.0, r, g, b)
);

color_int!(
    /// `ExtraCodecs.ARGB_COLOR_CODEC`: an int, or on read `[r, g, b, a]`.
    ArgbInt,
    4,
    |[r, g, b, a]: [f32; 4]| argb_from_floats(a, r, g, b)
);

/// `Codec.string(0, MAX_CHARS)`: bounded in UTF-16 code units.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Default)]
pub struct BoundedString<const MAX_CHARS: usize>(pub String);

impl<const MAX_CHARS: usize> BoundedString<MAX_CHARS> {
    pub fn new(text: impl Into<String>) -> anyhow::Result<Self> {
        let text = text.into();
        let chars = text.encode_utf16().count();
        ensure!(
            chars <= MAX_CHARS,
            "String \"{text}\" is too long: {chars}, expected range [0-{MAX_CHARS}]"
        );
        Ok(BoundedString(text))
    }
}

impl<const MAX_CHARS: usize> Serialize for BoundedString<MAX_CHARS> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.0)
    }
}

impl<'de, const MAX_CHARS: usize> Deserialize<'de> for BoundedString<MAX_CHARS> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        BoundedString::new(String::deserialize(d)?).map_err(D::Error::custom)
    }
}

impl<const MAX_CHARS: usize> Encode for BoundedString<MAX_CHARS> {
    fn encode(&self, w: impl Write) -> anyhow::Result<()> {
        Bounded::<&str, MAX_CHARS>(&self.0).encode(w)
    }
}

impl<const MAX_CHARS: usize> Decode<'_> for BoundedString<MAX_CHARS> {
    fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
        Ok(BoundedString(
            Bounded::<&str, MAX_CHARS>::decode(r)?.0.into(),
        ))
    }
}

impl<const MAX_CHARS: usize> EncodeCtx for BoundedString<MAX_CHARS> {
    fn encode_ctx(&self, _: &dyn RegistryLookup, w: impl Write) -> anyhow::Result<()> {
        self.encode(w)
    }
}

impl<const MAX_CHARS: usize> DecodeCtx<'_> for BoundedString<MAX_CHARS> {
    fn decode_ctx(_: &dyn RegistryLookup, r: &mut &[u8]) -> anyhow::Result<Self> {
        Self::decode(r)
    }
}

/// `ExtraCodecs.compactListCodec`: one element writes bare, any other count
/// writes a list; a bare element reads as a one-element list.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct CompactList<T>(pub Vec<T>);

impl<T: Serialize> Serialize for CompactList<T> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match &self.0[..] {
            [only] => only.serialize(s),
            list => list.serialize(s),
        }
    }
}

impl<'de, T: DeserializeOwned> Deserialize<'de> for CompactList<T> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct ListVisitor<T>(PhantomData<T>);

        impl<'de, T: DeserializeOwned> Visitor<'de> for ListVisitor<T> {
            type Value = CompactList<T>;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a list or a single element")
            }

            fn visit_seq<A: SeqAccess<'de>>(self, seq: A) -> Result<Self::Value, A::Error> {
                Vec::deserialize(value::SeqAccessDeserializer::new(seq)).map(CompactList)
            }

            fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<Self::Value, A::Error> {
                T::deserialize(value::MapAccessDeserializer::new(map)).map(|v| CompactList(vec![v]))
            }

            fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<Self::Value, E> {
                T::deserialize(value::StrDeserializer::new(v)).map(|v| CompactList(vec![v]))
            }

            fn visit_bool<E: serde::de::Error>(self, v: bool) -> Result<Self::Value, E> {
                T::deserialize(value::BoolDeserializer::new(v)).map(|v| CompactList(vec![v]))
            }

            fn visit_i64<E: serde::de::Error>(self, v: i64) -> Result<Self::Value, E> {
                T::deserialize(value::I64Deserializer::new(v)).map(|v| CompactList(vec![v]))
            }

            fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<Self::Value, E> {
                T::deserialize(value::U64Deserializer::new(v)).map(|v| CompactList(vec![v]))
            }

            fn visit_f64<E: serde::de::Error>(self, v: f64) -> Result<Self::Value, E> {
                T::deserialize(value::F64Deserializer::new(v)).map(|v| CompactList(vec![v]))
            }
        }

        d.deserialize_any(ListVisitor(PhantomData))
    }
}

impl<T: EncodeCtx> EncodeCtx for CompactList<T> {
    fn encode_ctx(&self, ctx: &dyn RegistryLookup, w: impl Write) -> anyhow::Result<()> {
        self.0.encode_ctx(ctx, w)
    }
}

impl<'a, T: DecodeCtx<'a>> DecodeCtx<'a> for CompactList<T> {
    fn decode_ctx(ctx: &dyn RegistryLookup, r: &mut &'a [u8]) -> anyhow::Result<Self> {
        Vec::decode_ctx(ctx, r).map(CompactList)
    }
}

/// `Codec.INT_STREAM` of a fixed length: a plain array in JSON, a
/// `TAG_Int_Array` in NBT and in the hash.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct IntArray<const N: usize>(pub [i32; N]);

impl<const N: usize> Default for IntArray<N> {
    fn default() -> Self {
        IntArray([0; N])
    }
}

impl<const N: usize> Serialize for IntArray<N> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        if s.is_human_readable() {
            self.0[..].serialize(s)
        } else {
            nbt_int_array(&self.0[..], s)
        }
    }
}

impl<'de, const N: usize> Deserialize<'de> for IntArray<N> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct Element(i32);

        impl<'de> Deserialize<'de> for Element {
            fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                int_value(d).map(Element)
            }
        }

        let values = Vec::<Element>::deserialize(d)?;
        let len = values.len();
        values
            .into_iter()
            .map(|Element(v)| v)
            .collect::<Vec<i32>>()
            .try_into()
            .map(IntArray)
            .map_err(|_| D::Error::custom(format_args!("expected {N} ints, got {len}")))
    }
}

/// `StatePropertiesPredicate` value: an exact value or a `{min, max}` range.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ValueMatcher {
    Exact(String),
    Ranged {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        min: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        max: Option<String>,
    },
}

impl EncodeCtx for ValueMatcher {
    fn encode_ctx(&self, _: &dyn RegistryLookup, mut w: impl Write) -> anyhow::Result<()> {
        match self {
            ValueMatcher::Exact(value) => {
                true.encode(&mut w)?;
                value.encode(w)
            }
            ValueMatcher::Ranged { min, max } => {
                false.encode(&mut w)?;
                min.encode(&mut w)?;
                max.encode(w)
            }
        }
    }
}

impl DecodeCtx<'_> for ValueMatcher {
    fn decode_ctx(_: &dyn RegistryLookup, r: &mut &[u8]) -> anyhow::Result<Self> {
        Ok(match bool::decode(r)? {
            true => ValueMatcher::Exact(String::decode(r)?),
            false => ValueMatcher::Ranged {
                min: Option::decode(r)?,
                max: Option::decode(r)?,
            },
        })
    }
}

/// `MinMaxBounds`: a bare number when both bounds agree, else `{min, max}`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct MinMaxBounds<T> {
    pub min: Option<T>,
    pub max: Option<T>,
}

impl<T> MinMaxBounds<T> {
    pub const ANY: Self = MinMaxBounds {
        min: None,
        max: None,
    };

    pub fn is_any(&self) -> bool {
        self.min.is_none() && self.max.is_none()
    }
}

/// A bound read as its `Codec` reads any number, ordered and printed as its
/// boxed Java type is.
pub trait Bound: Copy + Serialize {
    fn read<'de, D: Deserializer<'de>>(d: D) -> Result<Self, D::Error>;
    fn compare(self, other: Self) -> Ordering;
    fn java_string(self) -> String;
}

impl Bound for i32 {
    fn read<'de, D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        int_value(d)
    }

    fn compare(self, other: Self) -> Ordering {
        self.cmp(&other)
    }

    fn java_string(self) -> String {
        self.to_string()
    }
}

/// `Double.compareTo` and `Double.equals` order `-0.0` below `0.0`, unlike
/// the primitive operators.
impl Bound for f64 {
    fn read<'de, D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        f64::deserialize(d)
    }

    fn compare(self, other: Self) -> Ordering {
        self.total_cmp(&other)
    }

    fn java_string(self) -> String {
        mcrs_minecraft_nbt::snbt::java_double(self)
    }
}

impl<T: Bound> Serialize for MinMaxBounds<T> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        if let (Some(min), Some(max)) = (self.min, self.max)
            && min.compare(max) == Ordering::Equal
        {
            return min.serialize(s);
        }
        let mut map = s.serialize_map(None)?;
        if let Some(min) = &self.min {
            map.serialize_entry("min", min)?;
        }
        if let Some(max) = &self.max {
            map.serialize_entry("max", max)?;
        }
        map.end()
    }
}

fn optional_bound<'de, T: Bound, D: Deserializer<'de>>(d: D) -> Result<Option<T>, D::Error> {
    struct OptionalBound<T>(PhantomData<T>);

    impl<'de, T: Bound> Visitor<'de> for OptionalBound<T> {
        type Value = Option<T>;

        fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
            f.write_str("a number")
        }

        fn visit_none<E: serde::de::Error>(self) -> Result<Self::Value, E> {
            Ok(None)
        }

        fn visit_unit<E: serde::de::Error>(self) -> Result<Self::Value, E> {
            Ok(None)
        }

        fn visit_some<D: Deserializer<'de>>(self, d: D) -> Result<Self::Value, D::Error> {
            T::read(d).map(Some)
        }
    }

    d.deserialize_option(OptionalBound(PhantomData))
}

impl<'de, T: Bound> Deserialize<'de> for MinMaxBounds<T> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields, bound = "T: Bound")]
        struct Range<T> {
            #[serde(default, deserialize_with = "optional_bound")]
            min: Option<T>,
            #[serde(default, deserialize_with = "optional_bound")]
            max: Option<T>,
        }

        struct BoundsVisitor<T>(PhantomData<T>, bool);

        fn exactly<T: Bound, E: serde::de::Error>(number: Number) -> Result<MinMaxBounds<T>, E> {
            let exact = T::read(number).map_err(E::custom)?;
            Ok(MinMaxBounds {
                min: Some(exact),
                max: Some(exact),
            })
        }

        impl<'de, T: Bound> Visitor<'de> for BoundsVisitor<T> {
            type Value = MinMaxBounds<T>;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a number or a {min, max} range")
            }

            fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<Self::Value, A::Error> {
                let Range::<T> { min, max } =
                    Range::deserialize(value::MapAccessDeserializer::new(map))?;
                if let (Some(lo), Some(hi)) = (min, max)
                    && lo.compare(hi) == Ordering::Greater
                {
                    return Err(A::Error::custom(format_args!(
                        "Swapped bounds in range: Optional[{}] is higher than Optional[{}]",
                        lo.java_string(),
                        hi.java_string()
                    )));
                }
                Ok(MinMaxBounds { min, max })
            }

            fn visit_i64<E: serde::de::Error>(self, v: i64) -> Result<Self::Value, E> {
                exactly(Number::I64(v, self.1))
            }

            fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<Self::Value, E> {
                exactly(Number::U64(v, self.1))
            }

            fn visit_f64<E: serde::de::Error>(self, v: f64) -> Result<Self::Value, E> {
                exactly(Number::F64(v, self.1))
            }
        }

        let human_readable = d.is_human_readable();
        d.deserialize_any(BoundsVisitor(PhantomData, human_readable))
    }
}

/// `NbtPredicate`: a compound written as SNBT text, read from either.
#[derive(Clone, Debug, Default)]
pub struct NbtPredicate(pub NbtCompound);

/// `CompoundTag.equals` is map equality at every depth; the SNBT writer
/// sorts keys, so its text is that comparison.
impl PartialEq for NbtPredicate {
    fn eq(&self, other: &Self) -> bool {
        mcrs_minecraft_nbt::snbt::write_compound(&self.0)
            == mcrs_minecraft_nbt::snbt::write_compound(&other.0)
    }
}

impl Serialize for NbtPredicate {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&mcrs_minecraft_nbt::snbt::write_compound(&self.0))
    }
}

impl<'de> Deserialize<'de> for NbtPredicate {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        compound_or_snbt(d).map(NbtPredicate)
    }
}

ctx_free!(NbtPredicate);

impl Encode for NbtPredicate {
    fn encode(&self, w: impl Write) -> anyhow::Result<()> {
        self.0.encode(w)
    }
}

impl Decode<'_> for NbtPredicate {
    fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
        NbtCompound::decode(r).map(NbtPredicate)
    }
}

/// `MapCodec.unitCodec`: writes `{}` and reads any map, ignoring its fields.
pub fn serialize_unit<S: Serializer>(s: S) -> Result<S::Ok, S::Error> {
    s.serialize_map(Some(0))?.end()
}

pub fn deserialize_unit<'de, D: Deserializer<'de>>(d: D) -> Result<(), D::Error> {
    struct AnyMap;

    impl<'de> Visitor<'de> for AnyMap {
        type Value = ();

        fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
            f.write_str("a map")
        }

        fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<(), A::Error> {
            while map.next_entry::<IgnoredAny, IgnoredAny>()?.is_some() {}
            Ok(())
        }
    }

    d.deserialize_map(AnyMap)
}

/// A kind whose value carries no fields: `{}` persistent, nothing on the wire.
macro_rules! unit_component {
    ($($(#[$meta:meta])* $ty:ident),* $(,)?) => {$(
        $(#[$meta])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
        pub struct $ty;

        impl serde::Serialize for $ty {
            fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                $crate::item::component::serialize_unit(s)
            }
        }

        impl<'de> serde::Deserialize<'de> for $ty {
            fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                $crate::item::component::deserialize_unit(d).map(|()| $ty)
            }
        }

        impl $crate::item::ctx::EncodeCtx for $ty {
            fn encode_ctx(
                &self,
                _: &dyn mcrs_minecraft_registry::RegistryLookup,
                _: impl std::io::Write,
            ) -> anyhow::Result<()> {
                Ok(())
            }
        }

        impl $crate::item::ctx::DecodeCtx<'_> for $ty {
            fn decode_ctx(
                _: &dyn mcrs_minecraft_registry::RegistryLookup,
                _: &mut &[u8],
            ) -> anyhow::Result<Self> {
                Ok($ty)
            }
        }

        impl $crate::item::harness::Sample for $ty {
            fn nbt_tags(&self) -> Vec<(&'static str, u8)> {
                vec![("", mcrs_minecraft_nbt::COMPOUND_ID)]
            }

            fn samples() -> Vec<Self> {
                vec![$ty]
            }
        }
    )*};
}
pub(crate) use unit_component;

