pub mod id;
pub mod lerp;
pub mod mob_spawns;
pub mod modifier;
pub mod spec;
pub mod value;

use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

use serde::de::value::{
    BoolDeserializer, F64Deserializer, I64Deserializer, MapAccessDeserializer,
    SeqAccessDeserializer, StrDeserializer, StringDeserializer, U64Deserializer,
};
use serde::de::{DeserializeSeed, Error as _, MapAccess, SeqAccess, Visitor};
use serde::ser::{Error as _, SerializeMap};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use mcrs_minecraft_core::ResourceLocation;

pub use lerp::Lerp;
pub use mob_spawns::{MobSpawnSettings, SpawnCost};
pub use modifier::{ModifierError, Operation, apply};
pub use spec::{
    ArgumentRef, ArgumentSeed, AttributeError, AttributeRange, AttributeSpec, AttributeType,
    AttributeValue, ENVIRONMENT_ATTRIBUTES, attribute, is_syncable,
};
pub use value::{
    AmbientAdditions, AmbientMood, AmbientParticle, AmbientSounds, BackgroundMusic, BedRule,
    BedRuleCondition, MoonPhase, Music, TriState,
};

use spec::{MapMeaning, malformed};

/// `EnvironmentAttributeMap`: attribute id to a modifier applied to that attribute.
///
/// Each argument is read once, when the map is, into the type its attribute
/// registers; a frame borrows it from here.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct EnvironmentAttributeMap(pub BTreeMap<ResourceLocation<Arc<str>>, AttributeEntry>);

impl EnvironmentAttributeMap {
    pub fn get(&self, id: &str) -> Option<&AttributeEntry> {
        self.0.get(id)
    }

    pub fn argument(&self, id: &str) -> Option<&AttributeValue> {
        self.get(id).map(|entry| &entry.argument)
    }

    /// Checked against the attribute's type and modifier library, as a parsed
    /// entry is.
    pub fn modify(
        &mut self,
        id: ResourceLocation<&'static str>,
        modifier: Operation,
        argument: impl Serialize,
    ) -> Result<(), AttributeError> {
        let spec = attribute(id.as_str())
            .ok_or_else(|| AttributeError::UnknownAttribute(id.as_str().to_owned()))?;
        // Through the JSON text rather than a value tree: a tree widens an `f32`
        // to the nearest `f64` and so prints 0.07 as 0.07000000029802322.
        let refused = |error: serde_json::Error| malformed(spec.id, error.to_string());
        let text = serde_json::to_string(&argument).map_err(refused)?;
        let mut deserializer = serde_json::Deserializer::from_str(&text);
        let argument = spec
            .argument_seed(modifier)
            .deserialize(&mut deserializer)
            .map_err(refused)?;
        deserializer.end().map_err(refused)?;
        self.0
            .insert(id.into(), AttributeEntry { argument, modifier });
        Ok(())
    }

    pub fn filter_syncable(&self) -> Self {
        EnvironmentAttributeMap(
            self.0
                .iter()
                .filter(|(id, _)| is_syncable(id.as_str()))
                .map(|(id, entry)| (id.clone(), entry.clone()))
                .collect(),
        )
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// One entry of an [`EnvironmentAttributeMap`], mirroring `EnvironmentAttributeMap.Entry`.
///
/// The JSON has two shapes: a bare value, which implies the `override`
/// modifier, or `{"argument": …, "modifier": …}` naming the modifier
/// explicitly. Which one it is comes from the attribute's registered type, not
/// from the shape of the JSON.
#[derive(Debug, Clone, PartialEq)]
pub struct AttributeEntry {
    pub argument: AttributeValue,
    pub modifier: Operation,
}

impl AttributeEntry {
    pub fn override_value(argument: AttributeValue) -> Self {
        AttributeEntry {
            argument,
            modifier: Operation::Override,
        }
    }
}

/// The seed the map key hands its value.
///
/// The attribute's own value shape is tried first, so a value that is itself
/// an object can never be mistaken for the `{argument, modifier}` shape.
/// Streaming tells the two apart by what the source holds, and by the first
/// key where both are objects.
struct EntrySeed<'a>(&'a AttributeSpec);

impl<'de> DeserializeSeed<'de> for EntrySeed<'_> {
    type Value = AttributeEntry;

    fn deserialize<D: Deserializer<'de>>(
        self,
        deserializer: D,
    ) -> Result<AttributeEntry, D::Error> {
        deserializer.deserialize_any(EntryVisitor(self.0))
    }
}

struct EntryVisitor<'a>(&'a AttributeSpec);

impl EntryVisitor<'_> {
    fn value<'de, D: Deserializer<'de>>(
        &self,
        deserializer: D,
    ) -> Result<AttributeEntry, D::Error> {
        self.0
            .value_seed()
            .deserialize(deserializer)
            .map(AttributeEntry::override_value)
    }

    fn entry<'de, A: MapAccess<'de>>(
        &self,
        first_key: Option<String>,
        mut map: A,
    ) -> Result<AttributeEntry, A::Error> {
        let spec = self.0;
        let mut argument = None;
        let mut modifier = None;
        let mut pending = first_key;
        while let Some(key) = match pending.take() {
            Some(key) => Some(key),
            None => map.next_key::<String>()?,
        } {
            match key.as_str() {
                "argument" if argument.is_none() => {
                    argument = Some(map.next_value_seed(spec::DraftSeed(spec))?);
                }
                "modifier" if modifier.is_none() => {
                    modifier = Some(map.next_value::<Operation>().map_err(|error| {
                        A::Error::custom(malformed(spec.id, error.to_string()))
                    })?);
                }
                "argument" | "modifier" => {
                    return Err(A::Error::custom(malformed(
                        spec.id,
                        format!("entry repeats `{key}`"),
                    )));
                }
                other => {
                    return Err(A::Error::custom(malformed(
                        spec.id,
                        format!("entry has unexpected field `{other}`"),
                    )));
                }
            }
        }
        let modifier = modifier
            .ok_or_else(|| A::Error::custom(malformed(spec.id, "entry is missing `modifier`")))?;
        let argument = argument
            .ok_or_else(|| A::Error::custom(malformed(spec.id, "entry is missing `argument`")))?
            .finish(spec, modifier)
            .map_err(A::Error::custom)?;
        Ok(AttributeEntry { argument, modifier })
    }
}

impl<'de> Visitor<'de> for EntryVisitor<'_> {
    type Value = AttributeEntry;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "a {:?} value or a modifier entry for `{}`",
            self.0.ty, self.0.id
        )
    }

    fn visit_bool<E: serde::de::Error>(self, v: bool) -> Result<AttributeEntry, E> {
        self.value(BoolDeserializer::<E>::new(v))
    }

    fn visit_i64<E: serde::de::Error>(self, v: i64) -> Result<AttributeEntry, E> {
        self.value(I64Deserializer::<E>::new(v))
    }

    fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<AttributeEntry, E> {
        self.value(U64Deserializer::<E>::new(v))
    }

    fn visit_f64<E: serde::de::Error>(self, v: f64) -> Result<AttributeEntry, E> {
        self.value(F64Deserializer::<E>::new(v))
    }

    fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<AttributeEntry, E> {
        self.value(StrDeserializer::<E>::new(v))
    }

    fn visit_seq<A: SeqAccess<'de>>(self, seq: A) -> Result<AttributeEntry, A::Error> {
        self.value(SeqAccessDeserializer::new(seq))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<AttributeEntry, A::Error> {
        match self.0.ty.map_meaning() {
            MapMeaning::Value => self.value(MapAccessDeserializer::new(map)),
            MapMeaning::Entry => self.entry(None, map),
            MapMeaning::Peek => match map.next_key::<String>()? {
                Some(key) if key == "argument" || key == "modifier" => self.entry(Some(key), map),
                first => self.value(MapAccessDeserializer::new(Replay { first, rest: map })),
            },
        }
    }
}

struct Replay<A> {
    first: Option<String>,
    rest: A,
}

impl<'de, A: MapAccess<'de>> MapAccess<'de> for Replay<A> {
    type Error = A::Error;

    fn next_key_seed<K: DeserializeSeed<'de>>(
        &mut self,
        seed: K,
    ) -> Result<Option<K::Value>, A::Error> {
        match self.first.take() {
            Some(key) => seed.deserialize(StringDeserializer::new(key)).map(Some),
            None => self.rest.next_key_seed(seed),
        }
    }

    fn next_value_seed<V: DeserializeSeed<'de>>(&mut self, seed: V) -> Result<V::Value, A::Error> {
        self.rest.next_value_seed(seed)
    }
}

struct EntryRef<'a> {
    spec: &'a AttributeSpec,
    entry: &'a AttributeEntry,
}

impl Serialize for EntryRef<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let argument = ArgumentRef {
            spec: self.spec,
            op: self.entry.modifier,
            value: &self.entry.argument,
        };
        match self.entry.modifier {
            Operation::Override => argument.serialize(serializer),
            modifier => {
                let mut map = serializer.serialize_map(Some(2))?;
                map.serialize_entry("argument", &argument)?;
                map.serialize_entry("modifier", &modifier)?;
                map.end()
            }
        }
    }
}

impl Serialize for EnvironmentAttributeMap {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(self.0.len()))?;
        for (id, entry) in &self.0 {
            let spec = attribute(id.as_str()).ok_or_else(|| {
                S::Error::custom(AttributeError::UnknownAttribute(id.as_str().to_owned()))
            })?;
            map.serialize_entry(id, &EntryRef { spec, entry })?;
        }
        map.end()
    }
}

impl<'de> Deserialize<'de> for EnvironmentAttributeMap {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct MapVisitor;

        impl<'de> Visitor<'de> for MapVisitor {
            type Value = EnvironmentAttributeMap;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a map of environment attribute id to modifier entry")
            }

            fn visit_map<A: MapAccess<'de>>(
                self,
                mut map: A,
            ) -> Result<EnvironmentAttributeMap, A::Error> {
                let mut entries = BTreeMap::new();
                while let Some(id) = map.next_key::<ResourceLocation<Arc<str>>>()? {
                    let spec = attribute(id.as_str()).ok_or_else(|| {
                        A::Error::custom(AttributeError::UnknownAttribute(id.as_str().to_owned()))
                    })?;
                    entries.insert(id, map.next_value_seed(EntrySeed(spec))?);
                }
                Ok(EnvironmentAttributeMap(entries))
            }
        }

        deserializer.deserialize_map(MapVisitor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn round_trips_both_entry_shapes() {
        mcrs_minecraft_worldgen_testing::corpus_set().scope(|| {
            let json = json!({
                "minecraft:visual/sky_color": "#78a7ff",
                "minecraft:visual/water_fog_end_distance": {"argument": 0.85, "modifier": "multiply"},
                "minecraft:audio/background_music": {
                    "default": {"sound": "minecraft:music.game", "min_delay": 12000, "max_delay": 24000},
                },
                "minecraft:gameplay/increased_fire_burnout": true,
            });
            let map: EnvironmentAttributeMap = serde_json::from_value(json.clone()).unwrap();

            assert_eq!(
                map.get("minecraft:visual/sky_color").unwrap().modifier,
                Operation::Override
            );
            assert_eq!(
                map.get("minecraft:visual/water_fog_end_distance")
                    .unwrap()
                    .modifier,
                Operation::Multiply
            );
            // a value that is itself an object must not be mistaken for the full shape
            assert_eq!(
                map.get("minecraft:audio/background_music")
                    .unwrap()
                    .modifier,
                Operation::Override
            );

            let written: serde_json::Value =
                serde_json::from_str(&serde_json::to_string(&map).unwrap()).unwrap();
            assert_eq!(written, json);

            let syncable = map.filter_syncable();
            assert!(
                syncable
                    .get("minecraft:gameplay/increased_fire_burnout")
                    .is_none()
            );
            assert_eq!(syncable.0.len(), 3);
        });
    }

    fn tag_at<'a>(
        tag: &'a mcrs_minecraft_nbt::tag::NbtTag,
        path: &[&str],
    ) -> Option<&'a mcrs_minecraft_nbt::tag::NbtTag> {
        use mcrs_minecraft_nbt::tag::NbtTag;
        path.iter().try_fold(tag, |tag, key| match tag {
            NbtTag::Compound(compound) => compound
                .child_tags
                .iter()
                .find(|(name, _)| name == key)
                .map(|(_, child)| child),
            _ => None,
        })
    }

    #[test]
    fn background_music_is_typed_and_writes_ints() {
        mcrs_minecraft_worldgen_testing::corpus_set().scope(|| {
            use mcrs_minecraft_nbt::tag::NbtTag;

            let map: EnvironmentAttributeMap = serde_json::from_value(json!({
                "minecraft:audio/background_music": {
                    "default": {"sound": "minecraft:music.game", "min_delay": 12000, "max_delay": 24000},
                    "creative": {
                        "sound": "minecraft:music.creative",
                        "min_delay": 1,
                        "max_delay": 2,
                        "replace_current_music": true
                    },
                },
            }))
            .unwrap();

            let AttributeValue::BackgroundMusic(music) =
                map.argument("minecraft:audio/background_music").unwrap()
            else {
                panic!("background music is typed");
            };
            assert_eq!(music.default.as_ref().unwrap().min_delay.0, 12000);
            assert!(music.creative.as_ref().unwrap().replace_current_music);
            assert!(music.underwater.is_none());

            let tag = mcrs_minecraft_nbt::to_nbt_tag(&map).unwrap();
            let music = ["minecraft:audio/background_music"];
            let at = |rest: &[&str]| tag_at(&tag, &[&music[..], rest].concat());
            assert_eq!(at(&["default", "min_delay"]), Some(&NbtTag::Int(12000)));
            assert_eq!(at(&["default", "max_delay"]), Some(&NbtTag::Int(24000)));
            assert_eq!(at(&["creative", "min_delay"]), Some(&NbtTag::Int(1)));
            assert_eq!(
                at(&["creative", "replace_current_music"]),
                Some(&NbtTag::Byte(1))
            );
            assert_eq!(at(&["default", "replace_current_music"]), None);
        });
    }

    #[test]
    fn a_value_shape_and_an_entry_shape_are_told_apart() {
        let spawns = "minecraft:gameplay/natural_mob_spawns";
        let settings = json!({
            "spawn_costs": {"minecraft:zombie": {"charge": 1.0, "energy_budget": 0.5}},
            "spawns_by_category": {"monster": []},
        });
        for (shape, modifier) in [
            (settings.clone(), Operation::Override),
            (
                json!({"argument": settings.clone(), "modifier": "overlay"}),
                Operation::Overlay,
            ),
            (
                json!({"modifier": "overlay", "argument": settings.clone()}),
                Operation::Overlay,
            ),
        ] {
            let map: EnvironmentAttributeMap =
                serde_json::from_value(json!({ spawns: shape })).unwrap();
            let entry = map.get(spawns).unwrap();
            assert_eq!(entry.modifier, modifier);
            let AttributeValue::MobSpawns(read) = &entry.argument else {
                panic!("mob spawn settings are typed");
            };
            assert_eq!(read.spawn_costs.len(), 1);
            assert_eq!(read.spawns_by_category.len(), 1);
        }

        for (shape, missing) in [
            (json!({}), "spawn_costs"),
            (json!({"spawn_costs": {}}), "spawns_by_category"),
            (json!({"spawns_by_category": {}}), "spawn_costs"),
        ] {
            let error = serde_json::from_value::<EnvironmentAttributeMap>(json!({ spawns: shape }))
                .unwrap_err();
            assert!(
                error
                    .to_string()
                    .contains(&format!("missing field `{missing}`")),
                "{error}"
            );
        }

        let error = serde_json::from_value::<EnvironmentAttributeMap>(json!({
            spawns: {"modifier": "overlay"},
        }))
        .unwrap_err();
        assert!(error.to_string().contains("missing `argument`"), "{error}");
    }

    #[test]
    fn an_explicit_override_entry_reads_as_the_bare_value() {
        let mut checked = 0;
        for spec in ENVIRONMENT_ATTRIBUTES.values() {
            if matches!(
                spec.ty,
                AttributeType::BackgroundMusic | AttributeType::AmbientSounds
            ) {
                continue;
            }
            let bare = serde_json::to_value(ArgumentRef {
                spec,
                op: Operation::Override,
                value: &spec.default,
            })
            .unwrap();
            let read = |entry: serde_json::Value| {
                serde_json::from_value::<EnvironmentAttributeMap>(json!({ spec.id: entry }))
                    .unwrap_or_else(|error| panic!("{}: {error}", spec.id))
            };
            for entry in [
                json!({"argument": bare, "modifier": "override"}),
                json!({"modifier": "override", "argument": bare}),
            ] {
                assert_eq!(read(entry), read(bare.clone()), "{}", spec.id);
            }
            checked += 1;
        }
        assert!(checked > 40, "{checked} attributes");
    }

    #[test]
    fn an_object_valued_attribute_beats_the_entry_shape() {
        // `background_music` has no modifier but `override`, so a payload whose
        // keys look exactly like the entry shape is read as the value, and
        // refused for the fields it does not have.
        let error = serde_json::from_value::<EnvironmentAttributeMap>(json!({
            "minecraft:audio/background_music": {"argument": {}, "modifier": "override"},
        }))
        .unwrap_err();
        assert!(
            error.to_string().contains("unknown field `argument`"),
            "{error}"
        );
    }

    #[test]
    fn every_shipped_attribute_value_round_trips() {
        mcrs_minecraft_worldgen_testing::corpus_set().scope(|| {
            let mut maps: Vec<(String, serde_json::Value)> = Vec::new();
            for (path, file) in mcrs_minecraft_worldgen_testing::parse_all::<serde_json::Value>(
                "minecraft/dimension_type",
            ) {
                maps.push((path.display().to_string(), file["attributes"].clone()));
            }
            for (id, biome) in
                mcrs_minecraft_worldgen_testing::registry::<serde_json::Value>("biome")
            {
                maps.push((id.to_string(), biome["attributes"].clone()));
            }

            let mut entries = 0;
            for (name, attributes) in maps {
                if attributes.is_null() {
                    continue;
                }
                let typed: EnvironmentAttributeMap = serde_json::from_value(attributes.clone())
                    .unwrap_or_else(|error| panic!("{name}: {error}"));
                let written: serde_json::Value =
                    serde_json::from_str(&serde_json::to_string(&typed).unwrap()).unwrap();
                assert_eq!(written, attributes, "{name}");
                entries += typed.0.len();
            }
            assert!(entries > 200, "{entries} attribute entries");
        });
    }

    #[test]
    fn unknown_attributes_are_loud() {
        let err = serde_json::from_value::<EnvironmentAttributeMap>(json!({
            "minecraft:visual/sky_colour": "#78a7ff",
        }))
        .unwrap_err();
        assert!(
            err.to_string().contains("unknown environment attribute"),
            "{err}"
        );
    }

    #[test]
    fn invalid_modifiers_are_rejected() {
        let err = serde_json::from_value::<EnvironmentAttributeMap>(json!({
            "minecraft:gameplay/piglins_zombify": {"argument": true, "modifier": "multiply"},
        }))
        .unwrap_err();
        assert!(err.to_string().contains("not a valid modifier"), "{err}");
    }

    #[test]
    fn typed_values_are_borrowed_from_the_entry() {
        let map: EnvironmentAttributeMap = serde_json::from_value(json!({
            "minecraft:visual/sky_color": "#78a7ff",
        }))
        .unwrap();
        assert_eq!(
            map.get("minecraft:visual/sky_color").unwrap().argument,
            AttributeValue::Color(0xFF78_A7FF)
        );
    }

    #[test]
    fn a_modifier_after_its_argument_is_checked_when_it_arrives() {
        let read = |entry: serde_json::Value| {
            serde_json::from_value::<EnvironmentAttributeMap>(json!({
                "minecraft:visual/sky_color": entry,
            }))
        };
        let blended = read(json!({"argument": "#80102030", "modifier": "alpha_blend"})).unwrap();
        assert_eq!(
            blended.get("minecraft:visual/sky_color").unwrap().argument,
            AttributeValue::Color(0x8010_2030)
        );
        assert!(read(json!({"argument": "#80102030", "modifier": "add"})).is_err());
        assert!(read(json!({"modifier": "add", "argument": "#80102030"})).is_err());
    }
}
