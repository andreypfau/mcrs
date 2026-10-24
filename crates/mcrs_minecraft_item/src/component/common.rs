use mcrs_minecraft_core::Registered;
use std::fmt;
use std::marker::PhantomData;

pub use mcrs_minecraft_core::codec::CompactList;
use mcrs_minecraft_core::codec::{
    Bounded, NonNegativeInt, default_true, float_value, int_value, is_default, optional_flag,
};
use mcrs_minecraft_core::{ResourceKey, ResourceLocation};

use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_nbt::tag::NbtTag;
use mcrs_minecraft_nbt::{from_tag, nbt_flag};
use mcrs_minecraft_registries::{
    FromWire, Named, Namer, Ref, Refs, Resolve, ResolveError, Resolved, Resolver, Wire, WireNames,
};
use serde::de::{DeserializeOwned, Error as _, IgnoredAny, MapAccess, SeqAccess, Visitor, value};
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::keys::MobEffect;
use crate::loot::ContextFloatProvider;
use crate::loot::ContextIntProvider;

pub use mcrs_minecraft_core::codec::{
    ArgbInt, BoundedString, IntArray, Number, RgbInt, compound_or_snbt, lenient, lenient_float,
    unsigned_byte,
};

pub use mcrs_minecraft_block_predicate::state_properties::ValueMatcher;

/// `{raw, filtered?}`, read leniently from a bare value.
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

impl<T: Resolve> Resolve for Filterable<T> {
    type Named = Filterable<T::Named>;

    fn resolve<C: Resolver>(named: Self::Named, ctx: &C) -> Result<Self, ResolveError> {
        Ok(Filterable {
            raw: T::resolve(named.raw, ctx).map_err(|e| e.at("raw"))?,
            filtered: Option::resolve(named.filtered, ctx).map_err(|e| e.at("filtered"))?,
        })
    }

    fn name<C: Namer>(&self, names: &C) -> Result<Self::Named, ResolveError> {
        Ok(Filterable {
            raw: self.raw.name(names).map_err(|e| e.at("raw"))?,
            filtered: self.filtered.name(names).map_err(|e| e.at("filtered"))?,
        })
    }
}

impl<T: FromWire> FromWire for Filterable<T> {
    type Wire = Filterable<T::Wire>;

    fn from_wire<C: WireNames>(wire: Self::Wire, ctx: &C) -> Result<Self, ResolveError> {
        Ok(Filterable {
            raw: T::from_wire(wire.raw, ctx).map_err(|e| e.at("raw"))?,
            filtered: Option::from_wire(wire.filtered, ctx).map_err(|e| e.at("filtered"))?,
        })
    }

    fn to_wire<C: WireNames>(&self, ctx: &C) -> Result<Self::Wire, ResolveError> {
        Ok(Filterable {
            raw: self.raw.to_wire(ctx).map_err(|e| e.at("raw"))?,
            filtered: self.filtered.to_wire(ctx).map_err(|e| e.at("filtered"))?,
        })
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

/// Written as its key alone; only the wire form carries the value inline.
pub struct WireRef<M: Refs, R: Registered, V>(pub Ref<M, R, V>);

impl<M: Refs, R: Registered, V> Clone for WireRef<M, R, V> {
    fn clone(&self) -> Self {
        WireRef(self.0.clone())
    }
}

impl<M: Refs, R: Registered, V: PartialEq> PartialEq for WireRef<M, R, V> {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl<M: Refs, R: Registered, V: fmt::Debug> fmt::Debug for WireRef<M, R, V> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl<R: Registered, V> Serialize for WireRef<Named, R, V> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match &self.0 {
            Ref::Id(key) => key.serialize(s),
            Ref::Inline(_) => Err(serde::ser::Error::custom(
                "an inline value has no persistent form",
            )),
        }
    }
}

impl<'de, R: Registered, V> Deserialize<'de> for WireRef<Named, R, V> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        ResourceKey::deserialize(d).map(|key| WireRef(Ref::Id(key)))
    }
}

impl<R: Registered, V: Resolve<Named: Clone>> Resolve for WireRef<Resolved, R, V> {
    type Named = WireRef<Named, R, V::Named>;

    fn resolve<C: Resolver>(named: Self::Named, ctx: &C) -> Result<Self, ResolveError> {
        Ref::resolve(named.0, ctx).map(WireRef)
    }

    fn name<C: Namer>(&self, names: &C) -> Result<Self::Named, ResolveError> {
        self.0.name(names).map(WireRef)
    }
}

impl<R: Registered, V: FromWire<Wire: Clone>> FromWire for WireRef<Resolved, R, V> {
    type Wire = WireRef<Wire, R, V::Wire>;

    fn from_wire<C: WireNames>(wire: Self::Wire, ctx: &C) -> Result<Self, ResolveError> {
        Ref::from_wire(wire.0, ctx).map(WireRef)
    }

    fn to_wire<C: WireNames>(&self, ctx: &C) -> Result<Self::Wire, ResolveError> {
        self.0.to_wire(ctx).map(WireRef)
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
    };
}

resolvable!(
    ResolvableInt,
    i32,
    ContextIntProvider,
    "an int or a context int provider id",
    int_value
);
resolvable!(
    ResolvableFloat,
    f32,
    ContextFloatProvider,
    "a float or a context float provider id",
    float_value
);

#[mcrs_minecraft_registries::registry_value]
#[derive(mcrs_minecraft_registries::FromWire, Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MobEffectInstance<M: Refs = Resolved> {
    pub id: M::Id<MobEffect>,
    #[serde(flatten)]
    pub details: MobEffectDetails,
}

/// `show_icon` is resolved on read (absent means `show_particles`) and always
/// written.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(from = "MobEffectDetailsRepr")]
pub struct MobEffectDetails {
    #[serde(skip_serializing_if = "is_default")]
    pub amplifier: i8,
    #[serde(skip_serializing_if = "is_default")]
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

pub(crate) fn one() -> NonNegativeInt {
    Bounded(1)
}

pub(crate) fn is_one(value: &NonNegativeInt) -> bool {
    value.0 == 1
}

/// A map kept in the order read, refusing a repeated key.
pub(crate) fn serialize_entries<K: Serialize, V: Serialize, S: Serializer>(
    entries: &[(K, V)],
    s: S,
) -> Result<S::Ok, S::Error> {
    s.collect_map(entries.iter().map(|(key, value)| (key, value)))
}

/// A record reads a map and nothing else, where the derived visitor would
/// also take the fields as a sequence.
pub(crate) fn map_only<'de, D: Deserializer<'de>, T: Deserialize<'de>>(
    d: D,
) -> Result<T, D::Error> {
    struct MapOnly<T>(PhantomData<T>);

    impl<'de, T: Deserialize<'de>> Visitor<'de> for MapOnly<T> {
        type Value = T;

        fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
            f.write_str("a map")
        }

        fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<T, A::Error> {
            T::deserialize(value::MapAccessDeserializer::new(map))
        }
    }

    d.deserialize_map(MapOnly(PhantomData))
}

/// A compound whose `id` names the type, read from a compound or an SNBT
/// string; the `id` is lifted out and written back first.
#[derive(Clone, Debug, PartialEq)]
pub struct TypedEntityData<R> {
    pub id: ResourceKey<R>,
    pub tag: NbtCompound,
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

macro_rules! ordinal_enum {
    ($name:ident { $($variant:ident),* $(,)? }) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(rename_all = "snake_case")]
        pub enum $name {
            $($variant),*
        }

        impl $name {
            pub const ALL: &'static [Self] = &[$(Self::$variant),*];
        }

    };
}
pub(crate) use ordinal_enum;

ordinal_enum! {
    EquipmentSlotGroup { Any, Mainhand, Offhand, Hand, Feet, Legs, Chest, Head, Armor, Body, Saddle }
}

ordinal_enum! {
    ItemUseAnimation { None, Eat, Drink, Block, Bow, Trident, Crossbow, Spyglass, TootHorn, Brush, Bundle, Spear }
}

pub use mcrs_minecraft_value_provider::bounds::{Bound, MinMaxBounds};

/// A compound written as SNBT text, read from either.
#[derive(Clone, Debug, Default)]
pub struct NbtPredicate(pub NbtCompound);

/// Equality is map equality at every depth; the SNBT writer sorts keys, so
/// its text is that comparison.
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

/// Writes `{}` and reads any map, ignoring its fields.
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
                $crate::component::serialize_unit(s)
            }
        }

        impl<'de> serde::Deserialize<'de> for $ty {
            fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                $crate::component::deserialize_unit(d).map(|()| $ty)
            }
        }

        impl $crate::harness::Sample for $ty {
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

macro_rules! transparent_newtype {
    ($($ty:ident($inner:ty) => [$($derive:ident),*]),* $(,)?) => {$(
        #[derive($($derive,)* Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $ty(pub $inner);

        impl $crate::harness::Sample for $ty {
            fn nbt_tags(&self) -> Vec<(&'static str, u8)> {
                self.0.nbt_tags()
            }

            fn samples() -> Vec<Self> {
                <$inner>::samples().into_iter().map($ty).collect()
            }
        }
    )*};
}
pub(crate) use transparent_newtype;
