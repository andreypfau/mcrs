use std::fmt;
use std::marker::PhantomData;
use std::sync::Arc;

use serde::de::{self, MapAccess, SeqAccess, Visitor};
use serde::ser::SerializeSeq;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::material::PushReaction;
use crate::material::map::MapColor;
use mcrs_core::ResourceLocation;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BlockDefinitionFile {
    pub format_version: String,
    #[serde(rename = "minecraft:block")]
    pub block: BlockDefinition,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BlockDefinition {
    pub description: Description,
    #[serde(default)]
    pub components: Components,
    #[serde(default)]
    pub permutations: Vec<Permutation>,
    /// The dense escape hatch, indexed by `state_id - base_state_id`, for
    /// components that vary with every property of the block.
    #[serde(default, rename = "mcrs:state_components")]
    pub state_components: Option<Vec<Components>>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Description {
    pub identifier: ResourceLocation<Arc<str>>,
    #[serde(default)]
    pub properties: BlockProperties,
    pub base_state_id: u16,
    pub default_state_id: u16,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Permutation {
    pub condition: String,
    pub components: Components,
}

/// The block's properties in Java `StateDefinition` declaration order. The
/// order decides state ids, so it is carried as a sequence and never as a map.
#[derive(Debug, Default)]
pub struct BlockProperties(pub Vec<BlockProperty>);

#[derive(Debug)]
pub struct BlockProperty {
    pub name: Box<str>,
    pub values: Vec<PropertyValue>,
}

impl BlockProperties {
    pub fn index_of(&self, name: &str) -> Option<usize> {
        self.0.iter().position(|p| &*p.name == name)
    }

    pub fn state_count(&self) -> usize {
        self.0.iter().map(|p| p.values.len()).product()
    }
}

impl<'de> Deserialize<'de> for BlockProperties {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;

        impl<'de> Visitor<'de> for V {
            type Value = BlockProperties;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a map of block state property name to possible values")
            }

            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<BlockProperties, A::Error> {
                let mut properties: Vec<BlockProperty> = Vec::new();
                while let Some(name) = map.next_key::<Box<str>>()? {
                    if properties.iter().any(|p| p.name == name) {
                        return Err(de::Error::custom(format!("duplicate property `{name}`")));
                    }
                    let values: Vec<PropertyValue> = map.next_value()?;
                    if values.is_empty() {
                        return Err(de::Error::custom(format!(
                            "property `{name}` has no values"
                        )));
                    }
                    properties.push(BlockProperty { name, values });
                }
                Ok(BlockProperties(properties))
            }
        }

        d.deserialize_map(V)
    }
}

/// A property value keeps the JSON type it was dumped with: `"true"` and
/// `true` are different values, and so are `"3"` and `3`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum PropertyValue {
    Str(Box<str>),
    Int(i32),
    Bool(bool),
}

impl fmt::Display for PropertyValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PropertyValue::Str(s) => write!(f, "'{s}'"),
            PropertyValue::Int(i) => write!(f, "{i}"),
            PropertyValue::Bool(b) => write!(f, "{b}"),
        }
    }
}

impl<'de> Deserialize<'de> for PropertyValue {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;

        impl Visitor<'_> for V {
            type Value = PropertyValue;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a string, integer or boolean property value")
            }

            fn visit_bool<E: de::Error>(self, v: bool) -> Result<PropertyValue, E> {
                Ok(PropertyValue::Bool(v))
            }

            fn visit_i64<E: de::Error>(self, v: i64) -> Result<PropertyValue, E> {
                i32::try_from(v)
                    .map(PropertyValue::Int)
                    .map_err(|_| E::custom(format!("property value {v} is out of range")))
            }

            fn visit_u64<E: de::Error>(self, v: u64) -> Result<PropertyValue, E> {
                i32::try_from(v)
                    .map(PropertyValue::Int)
                    .map_err(|_| E::custom(format!("property value {v} is out of range")))
            }

            fn visit_str<E: de::Error>(self, v: &str) -> Result<PropertyValue, E> {
                Ok(PropertyValue::Str(v.into()))
            }
        }

        d.deserialize_any(V)
    }
}

impl Serialize for PropertyValue {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            PropertyValue::Str(v) => s.serialize_str(v),
            PropertyValue::Int(v) => s.serialize_i32(*v),
            PropertyValue::Bool(v) => s.serialize_bool(*v),
        }
    }
}

/// One box in Bedrock convention: sixteenths of a block, origin relative to
/// the block centre on X and Z and to the block bottom on Y.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BlockBox {
    pub origin: [f32; 3],
    pub size: [f32; 3],
}

impl BlockBox {
    pub const FULL_CUBE: BlockBox = BlockBox {
        origin: [-8.0, 0.0, -8.0],
        size: [16.0, 16.0, 16.0],
    };
}

/// `minecraft:collision_box` and friends: a boolean, one box, or an array of
/// boxes. `false` is no box at all and `true` is the full cube.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct BoxList(pub Vec<BlockBox>);

impl<'de> Deserialize<'de> for BoxList {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;

        impl<'de> Visitor<'de> for V {
            type Value = BoxList;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a boolean, a box, or an array of boxes")
            }

            fn visit_bool<E: de::Error>(self, v: bool) -> Result<BoxList, E> {
                Ok(BoxList(if v {
                    vec![BlockBox::FULL_CUBE]
                } else {
                    Vec::new()
                }))
            }

            fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<BoxList, A::Error> {
                let single = BlockBox::deserialize(de::value::MapAccessDeserializer::new(map))?;
                Ok(BoxList(vec![single]))
            }

            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<BoxList, A::Error> {
                let mut boxes = Vec::with_capacity(seq.size_hint().unwrap_or(1));
                while let Some(b) = seq.next_element()? {
                    boxes.push(b);
                }
                Ok(BoxList(boxes))
            }
        }

        d.deserialize_any(V)
    }
}

impl Serialize for BoxList {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut seq = s.serialize_seq(Some(self.0.len()))?;
        for b in &self.0 {
            seq.serialize_element(b)?;
        }
        seq.end()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DestructibleByExplosion {
    pub explosion_resistance: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RedstoneConductivity {
    pub redstone_conductor: bool,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FluidStateDef {
    pub fluid: ResourceLocation<Arc<str>>,
    pub level: u8,
    pub source: bool,
}

/// `BlockBehaviour.Properties.instrument` — the instrument a note block placed
/// above the block plays. Unrelated to the note block's own `instrument`
/// property, which is an ordinary block state property.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Instrument {
    Harp,
    Basedrum,
    Snare,
    Hat,
    Bass,
    Flute,
    Bell,
    Guitar,
    Chime,
    Xylophone,
    IronXylophone,
    CowBell,
    Didgeridoo,
    Bit,
    Banjo,
    Pling,
    Trumpet,
    TrumpetExposed,
    TrumpetOxidized,
    TrumpetWeathered,
    Zombie,
    Skeleton,
    Creeper,
    Dragon,
    WitherSkeleton,
    Piglin,
    CustomHead,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RenderShape {
    Invisible,
    Model,
}

impl<'de> Deserialize<'de> for MapColor {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = <&str>::deserialize(d)?;
        let hex = s
            .strip_prefix('#')
            .filter(|hex| hex.len() == 6)
            .ok_or_else(|| de::Error::custom(format!("expected a `#rrggbb` colour, got `{s}`")))?;
        let value = u32::from_str_radix(hex, 16)
            .map_err(|_| de::Error::custom(format!("expected a `#rrggbb` colour, got `{s}`")))?;
        Ok(MapColor::from_u32(value))
    }
}

impl Serialize for MapColor {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b))
    }
}

macro_rules! components {
    ($($name:literal => $field:ident : $ty:ty),+ $(,)?) => {
        /// Every component the corpus may carry, in either the block-wide map,
        /// a permutation, or a `mcrs:state_components` entry. An absent field
        /// is a component the corpus did not state at that level.
        #[derive(Debug, Clone, Default)]
        pub struct Components {
            $(pub $field: Option<$ty>,)+
        }

        impl Components {
            pub const NAMES: &'static [&'static str] = &[$($name),+];

            pub fn overlay(&mut self, other: &Components) {
                $(if other.$field.is_some() {
                    self.$field = other.$field.clone();
                })+
            }

            pub fn present(&self, out: &mut Vec<&'static str>) {
                $(if self.$field.is_some() {
                    out.push($name);
                })+
            }

            /// The first component this state leaves unstated that a state may
            /// not leave unstated.
            pub fn missing(&self) -> Option<&'static str> {
                $(if self.$field.is_none() && !OPTIONAL_COMPONENTS.contains(&$name) {
                    return Some($name);
                })+
                None
            }
        }

        impl<'de> Deserialize<'de> for Components {
            fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                struct V;

                impl<'de> Visitor<'de> for V {
                    type Value = Components;

                    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                        f.write_str("a block definition component map")
                    }

                    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Components, A::Error> {
                        let mut out = Components::default();
                        while let Some(key) = map.next_key::<String>()? {
                            match key.as_str() {
                                $($name => {
                                    if out.$field.is_some() {
                                        return Err(de::Error::custom(
                                            format!("duplicate component `{}`", $name)));
                                    }
                                    let value = map
                                        .next_value_seed(PhantomData::<$ty>)
                                        .map_err(|e| de::Error::custom(
                                            format!("component `{}`: {e}", $name)))?;
                                    out.$field = Some(value);
                                })+
                                unknown => {
                                    return Err(de::Error::custom(
                                        format!("unknown component `{unknown}`")));
                                }
                            }
                        }
                        Ok(out)
                    }
                }

                d.deserialize_map(V)
            }
        }
    };
}

components! {
    "minecraft:light_emission" => light_emission: u8,
    "minecraft:light_dampening" => light_dampening: u8,
    "minecraft:friction" => friction: f32,
    "minecraft:map_color" => map_color: MapColor,
    "minecraft:collision_box" => collision_box: BoxList,
    "minecraft:selection_box" => selection_box: BoxList,
    "minecraft:destructible_by_explosion" => destructible_by_explosion: DestructibleByExplosion,
    "minecraft:redstone_conductivity" => redstone_conductivity: RedstoneConductivity,
    "mcrs:hardness" => hardness: f32,
    "mcrs:requires_correct_tool_for_drops" => requires_correct_tool_for_drops: bool,
    "mcrs:is_air" => is_air: bool,
    "mcrs:replaceable" => replaceable: bool,
    "mcrs:ignited_by_lava" => ignited_by_lava: bool,
    "mcrs:push_reaction" => push_reaction: PushReaction,
    "mcrs:instrument" => instrument: Instrument,
    "mcrs:use_shape_for_light_occlusion" => use_shape_for_light_occlusion: bool,
    "mcrs:render_shape" => render_shape: RenderShape,
    "mcrs:fluid_state" => fluid_state: FluidStateDef,
    "mcrs:occlusion_shape" => occlusion_shape: BoxList,
    "mcrs:propagates_skylight_down" => propagates_skylight_down: bool,
    "mcrs:emissive_rendering" => emissive_rendering: bool,
    "mcrs:is_solid_render" => is_solid_render: bool,
    "mcrs:is_collision_shape_full_block" => is_collision_shape_full_block: bool,
    "mcrs:has_block_entity" => has_block_entity: bool,
    "mcrs:is_signal_source" => is_signal_source: bool,
    "mcrs:has_analog_output_signal" => has_analog_output_signal: bool,
}

/// The one component a state may leave unstated: a state that holds no fluid
/// carries no fluid state.
pub const OPTIONAL_COMPONENTS: &[&str] = &["mcrs:fluid_state"];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn property_values_keep_their_json_type() {
        let values: Vec<PropertyValue> = serde_json::from_str(r#"["true", true, "3", 3]"#).unwrap();
        assert_eq!(
            values,
            vec![
                PropertyValue::Str("true".into()),
                PropertyValue::Bool(true),
                PropertyValue::Str("3".into()),
                PropertyValue::Int(3),
            ]
        );
    }

    #[test]
    fn property_values_round_trip() {
        let json = r#"["north",true,3]"#;
        let values: Vec<PropertyValue> = serde_json::from_str(json).unwrap();
        assert_eq!(serde_json::to_string(&values).unwrap(), json);
    }

    #[test]
    fn box_list_reads_all_three_shapes() {
        let boxes: BoxList = serde_json::from_str("true").unwrap();
        assert_eq!(boxes.0, vec![BlockBox::FULL_CUBE]);
        let boxes: BoxList = serde_json::from_str("false").unwrap();
        assert!(boxes.0.is_empty());
        let boxes: BoxList =
            serde_json::from_str(r#"{"origin":[-2,0,-2],"size":[4,10,4]}"#).unwrap();
        assert_eq!(
            boxes.0,
            vec![BlockBox {
                origin: [-2.0, 0.0, -2.0],
                size: [4.0, 10.0, 4.0]
            }]
        );
        let boxes: BoxList =
            serde_json::from_str(r#"[{"origin":[-8,0,-8],"size":[16,16,16]}]"#).unwrap();
        assert_eq!(boxes.0, vec![BlockBox::FULL_CUBE]);
    }

    #[test]
    fn box_list_round_trips_as_an_array() {
        let json = r#"[{"origin":[-8.0,0.0,-8.0],"size":[16.0,16.0,16.0]}]"#;
        let boxes: BoxList = serde_json::from_str(json).unwrap();
        assert_eq!(serde_json::to_string(&boxes).unwrap(), json);
    }

    #[test]
    fn map_color_round_trips_as_hex() {
        let color: MapColor = serde_json::from_str(r##""#4040ff""##).unwrap();
        assert_eq!(
            color,
            MapColor {
                r: 0x40,
                g: 0x40,
                b: 0xff
            }
        );
        assert_eq!(serde_json::to_string(&color).unwrap(), r##""#4040ff""##);
    }

    #[test]
    fn unknown_component_is_an_error_naming_it() {
        let err = serde_json::from_str::<Components>(r#"{"mcrs:nonsense": 1}"#).unwrap_err();
        assert!(
            err.to_string()
                .contains("unknown component `mcrs:nonsense`"),
            "{err}"
        );
    }

    #[test]
    fn component_of_the_wrong_shape_is_an_error_naming_it() {
        let err = serde_json::from_str::<Components>(r#"{"minecraft:light_emission": "bright"}"#)
            .unwrap_err();
        assert!(
            err.to_string()
                .contains("component `minecraft:light_emission`"),
            "{err}"
        );
    }

    #[test]
    fn duplicate_component_is_an_error() {
        let err =
            serde_json::from_str::<Components>(r#"{"mcrs:is_air": true, "mcrs:is_air": false}"#)
                .unwrap_err();
        assert!(
            err.to_string()
                .contains("duplicate component `mcrs:is_air`"),
            "{err}"
        );
    }

    #[test]
    fn properties_keep_declaration_order() {
        let props: BlockProperties = serde_json::from_str(
            r#"{"facing":["north","south"],"half":["top","bottom"],"waterlogged":[true,false]}"#,
        )
        .unwrap();
        let names: Vec<&str> = props.0.iter().map(|p| &*p.name).collect();
        assert_eq!(names, vec!["facing", "half", "waterlogged"]);
        assert_eq!(props.state_count(), 8);
    }
}
