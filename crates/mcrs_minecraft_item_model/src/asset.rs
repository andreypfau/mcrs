use std::fmt;
use std::marker::PhantomData;

use mcrs_minecraft_core::ResourceKey;
use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_core::codec::{Validate, default_true, is_default};
use mcrs_minecraft_item_component::{
    ComponentPredicate, ComponentPredicateType, DimensionReg, DyeColor, EntityTypeReg,
    ItemComponentKind, ItemComponentValue, RgbInt, TrimMaterialReg,
};
use mcrs_minecraft_nbt::tag::NbtTag;
use serde::de::{DeserializeSeed, Error as _, MapAccess, SeqAccess, Visitor, value};
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::transform::Transformation;

fn one() -> f32 {
    1.0
}

fn is_one(value: &f32) -> bool {
    *value == 1.0
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClientItem {
    pub model: UnbakedItemModel,
    #[serde(default = "default_true", skip_serializing_if = "Clone::clone")]
    pub hand_animation_on_swap: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub oversized_in_gui: bool,
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub swap_animation_scale: f32,
}

impl ClientItem {
    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        let item: ClientItem = serde_json::from_slice(bytes).map_err(|error| error.to_string())?;
        item.validate()?;
        Ok(item)
    }
}

impl Validate for ClientItem {
    fn validate(&self) -> Result<(), String> {
        self.model.validate()
    }
}

// `flatten` buffers through serde's self-describing `Content`, which is only
// sound because these assets are JSON-only; `deny_unknown_fields` cannot join
// it, so an unknown key on one of the flattened objects passes silently.
// ponytail: a hand-written map visitor per flattened variant would reject it.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum UnbakedItemModel {
    #[serde(rename = "minecraft:empty", alias = "empty")]
    Empty,
    #[serde(rename = "minecraft:model", alias = "model")]
    Model {
        model: ResourceLocation,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        transformation: Option<Transformation>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        tints: Vec<TintSource>,
    },
    #[serde(rename = "minecraft:composite", alias = "composite")]
    Composite {
        models: Vec<UnbakedItemModel>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        transformation: Option<Transformation>,
    },
    #[serde(rename = "minecraft:condition", alias = "condition")]
    Condition {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        transformation: Option<Transformation>,
        #[serde(flatten)]
        property: ConditionProperty,
        on_true: Box<UnbakedItemModel>,
        on_false: Box<UnbakedItemModel>,
    },
    #[serde(rename = "minecraft:select", alias = "select")]
    Select {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        transformation: Option<Transformation>,
        #[serde(flatten)]
        switch: SelectSwitch,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        fallback: Option<Box<UnbakedItemModel>>,
    },
    #[serde(rename = "minecraft:range_dispatch", alias = "range_dispatch")]
    RangeDispatch {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        transformation: Option<Transformation>,
        #[serde(flatten)]
        property: RangeProperty,
        #[serde(default = "one", skip_serializing_if = "is_one")]
        scale: f32,
        entries: Vec<RangeEntry>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        fallback: Option<Box<UnbakedItemModel>>,
    },
    #[serde(rename = "minecraft:special", alias = "special")]
    Special {
        base: ResourceLocation,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        transformation: Option<Transformation>,
        model: SpecialModel,
    },
    #[serde(
        rename = "minecraft:bundle/selected_item",
        alias = "bundle/selected_item"
    )]
    BundleSelectedItem,
}

impl UnbakedItemModel {
    pub fn children(&self) -> Vec<&UnbakedItemModel> {
        match self {
            Self::Empty | Self::BundleSelectedItem | Self::Model { .. } | Self::Special { .. } => {
                Vec::new()
            }
            Self::Composite { models, .. } => models.iter().collect(),
            Self::Condition {
                on_true, on_false, ..
            } => vec![on_true, on_false],
            Self::Select {
                switch, fallback, ..
            } => switch
                .case_models()
                .into_iter()
                .chain(fallback.as_deref())
                .collect(),
            Self::RangeDispatch {
                entries, fallback, ..
            } => entries
                .iter()
                .map(|entry| &entry.model)
                .chain(fallback.as_deref())
                .collect(),
        }
    }

    /// Every node of the tree, this one first.
    pub fn walk(&self) -> Vec<&UnbakedItemModel> {
        let mut out = vec![self];
        let mut i = 0;
        while i < out.len() {
            out.extend(out[i].children());
            i += 1;
        }
        out
    }
}

impl Validate for UnbakedItemModel {
    fn validate(&self) -> Result<(), String> {
        for node in self.walk() {
            match node {
                Self::Select { switch, .. } => switch.validate()?,
                Self::Model { tints, .. } => {
                    for tint in tints {
                        tint.validate()?;
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RangeEntry {
    pub threshold: f32,
    pub model: UnbakedItemModel,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "property")]
pub enum ConditionProperty {
    #[serde(rename = "minecraft:damaged", alias = "damaged")]
    Damaged,
    #[serde(rename = "minecraft:broken", alias = "broken")]
    Broken,
    #[serde(rename = "minecraft:has_component", alias = "has_component")]
    HasComponent {
        component: ItemComponentKind,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        ignore_default: bool,
    },
    #[serde(rename = "minecraft:component", alias = "component")]
    Component(ComponentMatches),
    #[serde(rename = "minecraft:custom_model_data", alias = "custom_model_data")]
    CustomModelData {
        #[serde(default, skip_serializing_if = "is_default")]
        index: u32,
    },
    #[serde(rename = "minecraft:using_item", alias = "using_item")]
    UsingItem,
    #[serde(rename = "minecraft:selected", alias = "selected")]
    Selected,
    #[serde(rename = "minecraft:carried", alias = "carried")]
    Carried,
    #[serde(rename = "minecraft:extended_view", alias = "extended_view")]
    ExtendedView,
    #[serde(rename = "minecraft:keybind_down", alias = "keybind_down")]
    KeybindDown { keybind: String },
    #[serde(rename = "minecraft:view_entity", alias = "view_entity")]
    ViewEntity,
    #[serde(rename = "minecraft:fishing_rod/cast", alias = "fishing_rod/cast")]
    FishingRodCast,
    #[serde(
        rename = "minecraft:bundle/has_selected_item",
        alias = "bundle/has_selected_item"
    )]
    BundleHasSelectedItem,
}

/// `{ predicate: <type>, value: <predicate of that type> }`.
#[derive(Debug, Clone, PartialEq)]
pub struct ComponentMatches {
    pub predicate: ComponentPredicate,
}

impl Serialize for ComponentMatches {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut map = s.serialize_map(Some(2))?;
        map.serialize_entry("predicate", self.predicate.kind().id().as_str())?;
        map.serialize_entry("value", &self.predicate)?;
        map.end()
    }
}

impl<'de> Deserialize<'de> for ComponentMatches {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let predicate = d.deserialize_map(KeyedPair {
            kind_key: "predicate",
            value_key: "value",
            parse_kind: |id: &str| {
                ComponentPredicateType::from_id(id)
                    .ok_or_else(|| format!("unknown data component predicate type `{id}`"))
            },
            seed: |kind: ComponentPredicateType| kind,
        })?;
        Ok(ComponentMatches { predicate })
    }
}

/// A map whose value's codec is chosen by a sibling key. The value is
/// buffered when it arrives first.
struct KeyedPair<P, F> {
    kind_key: &'static str,
    value_key: &'static str,
    parse_kind: P,
    seed: F,
}

impl<'de, K, V, P, F, S> Visitor<'de> for KeyedPair<P, F>
where
    P: Fn(&str) -> Result<K, String>,
    F: Fn(K) -> S,
    S: DeserializeSeed<'de, Value = V>,
{
    type Value = V;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "a map with `{}` and `{}`", self.kind_key, self.value_key)
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<V, A::Error> {
        let mut kind: Option<K> = None;
        let mut value: Option<V> = None;
        let mut buffered: Option<NbtTag> = None;
        while let Some(key) = map.next_key::<String>()? {
            match key.as_str() {
                k if k == self.kind_key => {
                    let id: String = map.next_value()?;
                    kind = Some((self.parse_kind)(&id).map_err(A::Error::custom)?);
                }
                k if k == self.value_key => match kind.take() {
                    Some(kind) => value = Some(map.next_value_seed((self.seed)(kind))?),
                    None => buffered = Some(map.next_value()?),
                },
                other => {
                    return Err(A::Error::custom(format_args!("unknown field `{other}`")));
                }
            }
        }
        if let Some(value) = value {
            return Ok(value);
        }
        let kind = kind.ok_or_else(|| A::Error::missing_field(self.kind_key))?;
        let tag = buffered.ok_or_else(|| A::Error::missing_field(self.value_key))?;
        (self.seed)(kind)
            .deserialize(tag)
            .map_err(|error| A::Error::custom(error.to_string()))
    }
}

/// `when` and `model`, `when` read through the seed the switch chose.
#[derive(Debug, Clone)]
pub struct Case<T> {
    pub when: Vec<T>,
    pub model: UnbakedItemModel,
}

impl<T: Serialize> Serialize for Case<T> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut map = s.serialize_map(Some(2))?;
        match &self.when[..] {
            [only] => map.serialize_entry("when", only)?,
            list => map.serialize_entry("when", list)?,
        }
        map.serialize_entry("model", &self.model)?;
        map.end()
    }
}

impl<'de, T: Deserialize<'de>> Deserialize<'de> for Case<T> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        CaseSeed(PhantomData::<T>).deserialize(d)
    }
}

#[derive(Clone)]
struct CaseSeed<S>(S);

impl<'de, S: DeserializeSeed<'de> + Clone> DeserializeSeed<'de> for CaseSeed<S> {
    type Value = Case<S::Value>;

    fn deserialize<D: Deserializer<'de>>(self, d: D) -> Result<Self::Value, D::Error> {
        d.deserialize_map(self)
    }
}

impl<'de, S: DeserializeSeed<'de> + Clone> Visitor<'de> for CaseSeed<S> {
    type Value = Case<S::Value>;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("a case with `when` and `model`")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
        let mut when = None;
        let mut model = None;
        while let Some(key) = map.next_key::<String>()? {
            match key.as_str() {
                "when" => when = Some(map.next_value_seed(CompactSeed(self.0.clone()))?),
                "model" => model = Some(map.next_value()?),
                other => return Err(A::Error::unknown_field(other, &["when", "model"])),
            }
        }
        let when: Vec<S::Value> = when.ok_or_else(|| A::Error::missing_field("when"))?;
        if when.is_empty() {
            return Err(A::Error::custom("List must have contents"));
        }
        Ok(Case {
            when,
            model: model.ok_or_else(|| A::Error::missing_field("model"))?,
        })
    }
}

/// A list of seeded elements, or one bare element.
struct CompactSeed<S>(S);

impl<'de, S: DeserializeSeed<'de> + Clone> DeserializeSeed<'de> for CompactSeed<S> {
    type Value = Vec<S::Value>;

    fn deserialize<D: Deserializer<'de>>(self, d: D) -> Result<Self::Value, D::Error> {
        d.deserialize_any(self)
    }
}

impl<'de, S: DeserializeSeed<'de> + Clone> Visitor<'de> for CompactSeed<S> {
    type Value = Vec<S::Value>;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("a list or a single element")
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
        let mut out = Vec::new();
        while let Some(element) = seq.next_element_seed(self.0.clone())? {
            out.push(element);
        }
        Ok(out)
    }

    fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<Self::Value, A::Error> {
        self.0
            .deserialize(value::MapAccessDeserializer::new(map))
            .map(|v| vec![v])
    }

    fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<Self::Value, E> {
        self.0
            .deserialize(value::StrDeserializer::new(v))
            .map(|v| vec![v])
    }

    fn visit_bool<E: serde::de::Error>(self, v: bool) -> Result<Self::Value, E> {
        self.0
            .deserialize(value::BoolDeserializer::new(v))
            .map(|v| vec![v])
    }

    fn visit_i64<E: serde::de::Error>(self, v: i64) -> Result<Self::Value, E> {
        self.0
            .deserialize(value::I64Deserializer::new(v))
            .map(|v| vec![v])
    }

    fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<Self::Value, E> {
        self.0
            .deserialize(value::U64Deserializer::new(v))
            .map(|v| vec![v])
    }

    fn visit_f64<E: serde::de::Error>(self, v: f64) -> Result<Self::Value, E> {
        self.0
            .deserialize(value::F64Deserializer::new(v))
            .map(|v| vec![v])
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "property")]
pub enum SelectSwitch {
    #[serde(rename = "minecraft:trim_material", alias = "trim_material")]
    TrimMaterial {
        cases: Vec<Case<ResourceKey<TrimMaterialReg>>>,
    },
    #[serde(rename = "minecraft:display_context", alias = "display_context")]
    DisplayContext { cases: Vec<Case<DisplayContext>> },
    #[serde(rename = "minecraft:block_state", alias = "block_state")]
    BlockState {
        block_state_property: String,
        cases: Vec<Case<String>>,
    },
    #[serde(rename = "minecraft:charge_type", alias = "charge_type")]
    ChargeType { cases: Vec<Case<ChargeType>> },
    #[serde(rename = "minecraft:custom_model_data", alias = "custom_model_data")]
    CustomModelData {
        #[serde(default, skip_serializing_if = "is_default")]
        index: u32,
        cases: Vec<Case<String>>,
    },
    #[serde(rename = "minecraft:main_hand", alias = "main_hand")]
    MainHand { cases: Vec<Case<HumanoidArm>> },
    #[serde(rename = "minecraft:local_time", alias = "local_time")]
    LocalTime {
        pattern: String,
        #[serde(default, skip_serializing_if = "String::is_empty")]
        locale: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        time_zone: Option<String>,
        cases: Vec<Case<String>>,
    },
    #[serde(
        rename = "minecraft:context_entity_type",
        alias = "context_entity_type"
    )]
    ContextEntityType {
        cases: Vec<Case<ResourceKey<EntityTypeReg>>>,
    },
    #[serde(rename = "minecraft:context_dimension", alias = "context_dimension")]
    ContextDimension {
        cases: Vec<Case<ResourceKey<DimensionReg>>>,
    },
    #[serde(rename = "minecraft:component", alias = "component")]
    Component(ComponentSwitch),
}

impl SelectSwitch {
    pub fn case_models(&self) -> Vec<&UnbakedItemModel> {
        fn models<T>(cases: &[Case<T>]) -> Vec<&UnbakedItemModel> {
            cases.iter().map(|case| &case.model).collect()
        }
        match self {
            Self::TrimMaterial { cases } => models(cases),
            Self::DisplayContext { cases } => models(cases),
            Self::BlockState { cases, .. } => models(cases),
            Self::ChargeType { cases } => models(cases),
            Self::CustomModelData { cases, .. } => models(cases),
            Self::MainHand { cases } => models(cases),
            Self::LocalTime { cases, .. } => models(cases),
            Self::ContextEntityType { cases } => models(cases),
            Self::ContextDimension { cases } => models(cases),
            Self::Component(switch) => models(&switch.cases),
        }
    }
}

fn validate_cases<T: PartialEq + fmt::Debug>(cases: &[Case<T>]) -> Result<(), String> {
    if cases.is_empty() {
        return Err("Empty case list".to_owned());
    }
    let values: Vec<&T> = cases.iter().flat_map(|case| &case.when).collect();
    let duplicates: Vec<String> = values
        .iter()
        .enumerate()
        .filter(|(i, v)| !values[..*i].contains(v) && values[i + 1..].contains(v))
        .map(|(_, v)| format!("{v:?}"))
        .collect();
    if duplicates.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "Duplicate case conditions: {}",
            duplicates.join(", ")
        ))
    }
}

impl Validate for SelectSwitch {
    fn validate(&self) -> Result<(), String> {
        match self {
            Self::TrimMaterial { cases } => validate_cases(cases),
            Self::DisplayContext { cases } => validate_cases(cases),
            Self::BlockState { cases, .. } => validate_cases(cases),
            Self::ChargeType { cases } => validate_cases(cases),
            Self::CustomModelData { cases, .. } => validate_cases(cases),
            Self::MainHand { cases } => validate_cases(cases),
            Self::LocalTime { cases, .. } => validate_cases(cases),
            Self::ContextEntityType { cases } => validate_cases(cases),
            Self::ContextDimension { cases } => validate_cases(cases),
            Self::Component(switch) => validate_cases(&switch.cases),
        }
    }
}

/// `{ component: <kind>, cases: [...] }` where each `when` is read with the
/// kind's own codec.
#[derive(Debug, Clone)]
pub struct ComponentSwitch {
    pub component: ItemComponentKind,
    pub cases: Vec<Case<ItemComponentValue>>,
}

impl Serialize for ComponentSwitch {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut map = s.serialize_map(Some(2))?;
        map.serialize_entry("component", self.component.id().as_str())?;
        map.serialize_entry("cases", &self.cases)?;
        map.end()
    }
}

impl<'de> Deserialize<'de> for ComponentSwitch {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let (component, cases) = d.deserialize_map(KeyedPair {
            kind_key: "component",
            value_key: "cases",
            parse_kind: |id: &str| {
                let kind = ItemComponentKind::from_id(id)
                    .ok_or_else(|| format!("unknown data component `{id}`"))?;
                if !kind.is_persistent() {
                    return Err("Component can't be serialized".to_owned());
                }
                Ok(kind)
            },
            seed: CasesSeed,
        })?;
        Ok(ComponentSwitch { component, cases })
    }
}

struct CasesSeed(ItemComponentKind);

impl<'de> DeserializeSeed<'de> for CasesSeed {
    type Value = (ItemComponentKind, Vec<Case<ItemComponentValue>>);

    fn deserialize<D: Deserializer<'de>>(self, d: D) -> Result<Self::Value, D::Error> {
        struct Cases(ItemComponentKind);

        impl<'de> Visitor<'de> for Cases {
            type Value = Vec<Case<ItemComponentValue>>;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a list of cases")
            }

            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
                let mut out = Vec::new();
                while let Some(case) = seq.next_element_seed(CaseSeed(self.0))? {
                    out.push(case);
                }
                Ok(out)
            }
        }

        d.deserialize_seq(Cases(self.0))
            .map(|cases| (self.0, cases))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "property")]
pub enum RangeProperty {
    #[serde(rename = "minecraft:damage", alias = "damage")]
    Damage {
        #[serde(default = "default_true", skip_serializing_if = "Clone::clone")]
        normalize: bool,
    },
    #[serde(rename = "minecraft:count", alias = "count")]
    Count {
        #[serde(default = "default_true", skip_serializing_if = "Clone::clone")]
        normalize: bool,
    },
    #[serde(rename = "minecraft:custom_model_data", alias = "custom_model_data")]
    CustomModelData {
        #[serde(default, skip_serializing_if = "is_default")]
        index: u32,
    },
    #[serde(rename = "minecraft:cooldown", alias = "cooldown")]
    Cooldown,
    #[serde(rename = "minecraft:bundle/fullness", alias = "bundle/fullness")]
    BundleFullness,
    #[serde(rename = "minecraft:crossbow/pull", alias = "crossbow/pull")]
    CrossbowPull,
    #[serde(rename = "minecraft:use_duration", alias = "use_duration")]
    UseDuration {
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        remaining: bool,
    },
    #[serde(rename = "minecraft:use_cycle", alias = "use_cycle")]
    UseCycle {
        #[serde(default = "one", skip_serializing_if = "is_one")]
        period: f32,
    },
    #[serde(rename = "minecraft:time", alias = "time")]
    Time {
        #[serde(default = "default_true", skip_serializing_if = "Clone::clone")]
        wobble: bool,
        source: TimeSource,
    },
    #[serde(rename = "minecraft:compass", alias = "compass")]
    Compass {
        #[serde(default = "default_true", skip_serializing_if = "Clone::clone")]
        wobble: bool,
        target: CompassTarget,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TimeSource {
    Random,
    Daytime,
    MoonPhase,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompassTarget {
    None,
    Lodestone,
    Spawn,
    Recovery,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", deny_unknown_fields)]
pub enum TintSource {
    #[serde(rename = "minecraft:constant", alias = "constant")]
    Constant { value: RgbInt },
    #[serde(rename = "minecraft:dye", alias = "dye")]
    Dye { default: RgbInt },
    #[serde(rename = "minecraft:potion", alias = "potion")]
    Potion { default: RgbInt },
    #[serde(rename = "minecraft:firework", alias = "firework")]
    Firework { default: RgbInt },
    #[serde(rename = "minecraft:grass", alias = "grass")]
    Grass { temperature: f32, downfall: f32 },
    #[serde(rename = "minecraft:custom_model_data", alias = "custom_model_data")]
    CustomModelData {
        #[serde(default, skip_serializing_if = "is_default")]
        index: u32,
        default: RgbInt,
    },
    #[serde(rename = "minecraft:team", alias = "team")]
    Team { default: RgbInt },
}

impl Validate for TintSource {
    fn validate(&self) -> Result<(), String> {
        if let Self::Grass {
            temperature,
            downfall,
        } = self
        {
            for (name, value) in [("temperature", *temperature), ("downfall", *downfall)] {
                if !(0.0..=1.0).contains(&value) {
                    return Err(format!("{name} {value} is outside [0, 1]"));
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", deny_unknown_fields)]
pub enum SpecialModel {
    #[serde(rename = "minecraft:bell", alias = "bell")]
    Bell,
    #[serde(rename = "minecraft:banner", alias = "banner")]
    Banner {
        color: DyeColor,
        #[serde(default, skip_serializing_if = "is_default")]
        attachment: BannerAttachment,
    },
    #[serde(rename = "minecraft:book", alias = "book")]
    Book {
        open_angle: f32,
        page1: f32,
        page2: f32,
    },
    #[serde(rename = "minecraft:conduit", alias = "conduit")]
    Conduit,
    #[serde(rename = "minecraft:chest", alias = "chest")]
    Chest {
        texture: ResourceLocation,
        #[serde(default, skip_serializing_if = "is_default")]
        openness: f32,
        #[serde(default, skip_serializing_if = "is_default")]
        chest_type: ChestType,
    },
    #[serde(
        rename = "minecraft:copper_golem_statue",
        alias = "copper_golem_statue"
    )]
    CopperGolemStatue {
        texture: ResourceLocation,
        pose: StatuePose,
    },
    #[serde(rename = "minecraft:head", alias = "head")]
    Head {
        kind: HeadKind,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        texture: Option<ResourceLocation>,
        #[serde(default, skip_serializing_if = "is_default")]
        animation: f32,
    },
    #[serde(rename = "minecraft:player_head", alias = "player_head")]
    PlayerHead,
    #[serde(rename = "minecraft:shulker_box", alias = "shulker_box")]
    ShulkerBox {
        texture: ResourceLocation,
        #[serde(default, skip_serializing_if = "is_default")]
        openness: f32,
    },
    #[serde(rename = "minecraft:shield", alias = "shield")]
    Shield,
    #[serde(rename = "minecraft:trident", alias = "trident")]
    Trident,
    #[serde(rename = "minecraft:decorated_pot", alias = "decorated_pot")]
    DecoratedPot,
    #[serde(rename = "minecraft:end_cube", alias = "end_cube")]
    EndCube { effect: EndCubeEffect },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BannerAttachment {
    #[default]
    Ground,
    Wall,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChestType {
    #[default]
    Single,
    Left,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StatuePose {
    Standing,
    Sitting,
    Running,
    Star,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HeadKind {
    Skeleton,
    WitherSkeleton,
    Player,
    Zombie,
    Creeper,
    Piglin,
    Dragon,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EndCubeEffect {
    Portal,
    Gateway,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[repr(u8)]
pub enum DisplayContext {
    None,
    ThirdpersonLefthand,
    ThirdpersonRighthand,
    FirstpersonLefthand,
    FirstpersonRighthand,
    Head,
    Gui,
    Ground,
    Fixed,
    OnShelf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChargeType {
    None,
    Arrow,
    Rocket,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HumanoidArm {
    Left,
    Right,
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use serde_json::Value;

    use super::*;

    fn item_stem(name: &str) -> Option<&str> {
        name.strip_prefix("minecraft/items/")?
            .strip_suffix(".json")
            .filter(|stem| !stem.contains('/'))
    }

    fn corpus() -> Vec<(String, Vec<u8>)> {
        let progress = mcrs_minecraft_client_jar::Progress::default();
        let (files, _) =
            mcrs_minecraft_client_jar::resolve(&progress, |name| item_stem(name).is_some(), drop);
        let mut files: Vec<(String, Vec<u8>)> = files
            .into_iter()
            .map(|(name, bytes)| (format!("minecraft:{}", item_stem(&name).unwrap()), bytes))
            .collect();
        files.sort();
        files
    }

    /// Equal up to number spelling: the assets are read as `f32`, so `0.58`
    /// written back is compared as the float it became.
    fn structurally_equal(a: &Value, b: &Value) -> bool {
        match (a, b) {
            (Value::Number(a), Value::Number(b)) => {
                a.as_f64().map(|v| v as f32) == b.as_f64().map(|v| v as f32)
            }
            (Value::Array(a), Value::Array(b)) => {
                a.len() == b.len() && a.iter().zip(b).all(|(a, b)| structurally_equal(a, b))
            }
            (Value::Object(a), Value::Object(b)) => {
                a.len() == b.len()
                    && a.iter()
                        .all(|(k, v)| b.get(k).is_some_and(|w| structurally_equal(v, w)))
            }
            _ => a == b,
        }
    }

    fn json_str<T: Serialize>(v: &T, key: &str) -> String {
        match serde_json::to_value(v).unwrap() {
            Value::Object(map) => map[key].as_str().unwrap().to_owned(),
            other => panic!("{other}"),
        }
    }

    #[test]
    fn every_item_model_asset_reads_and_writes_back_structurally_equal() {
        let mut files = 0;
        let mut kinds: BTreeMap<String, usize> = BTreeMap::new();
        let mut properties: BTreeMap<String, usize> = BTreeMap::new();
        let mut tints: BTreeMap<String, usize> = BTreeMap::new();
        let mut swap_scales = 0;
        for (id, bytes) in &corpus() {
            files += 1;
            let item = ClientItem::parse(bytes).unwrap_or_else(|error| panic!("{id}: {error}"));
            let original: Value = serde_json::from_slice(bytes).unwrap();
            let written = serde_json::to_value(&item).unwrap();
            assert!(
                structurally_equal(&original, &written),
                "{id} did not round-trip:\n{original}\n{written}"
            );
            if item.swap_animation_scale != 1.0 {
                swap_scales += 1;
            }
            for node in item.model.walk() {
                *kinds.entry(json_str(node, "type")).or_default() += 1;
                match node {
                    UnbakedItemModel::Condition { property, .. } => {
                        let name = format!("condition {}", json_str(property, "property"));
                        *properties.entry(name).or_default() += 1;
                    }
                    UnbakedItemModel::Select { switch, .. } => {
                        let name = format!("select {}", json_str(switch, "property"));
                        *properties.entry(name).or_default() += 1;
                    }
                    UnbakedItemModel::RangeDispatch { property, .. } => {
                        let name = format!("range {}", json_str(property, "property"));
                        *properties.entry(name).or_default() += 1;
                    }
                    UnbakedItemModel::Model { tints: sources, .. } => {
                        for tint in sources {
                            *tints.entry(json_str(tint, "type")).or_default() += 1;
                        }
                    }
                    _ => {}
                }
            }
        }
        assert_eq!(files, 1658);
        assert_eq!(swap_scales, 7);
        let expected_kinds: BTreeMap<String, usize> = [
            ("minecraft:model", 2253),
            ("minecraft:special", 91),
            ("minecraft:select", 71),
            ("minecraft:composite", 34),
            ("minecraft:condition", 26),
            ("minecraft:bundle/selected_item", 17),
            ("minecraft:range_dispatch", 8),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_owned(), v))
        .collect();
        assert_eq!(kinds, expected_kinds);
        let expected_properties: BTreeMap<String, usize> = [
            ("select minecraft:trim_material", 29),
            ("select minecraft:display_context", 26),
            ("condition minecraft:bundle/has_selected_item", 17),
            ("select minecraft:block_state", 12),
            ("condition minecraft:using_item", 5),
            ("range minecraft:compass", 3),
            ("range minecraft:time", 2),
            ("select minecraft:local_time", 2),
            ("condition minecraft:has_component", 2),
            ("select minecraft:context_dimension", 1),
            ("range minecraft:use_cycle", 1),
            ("condition minecraft:fishing_rod/cast", 1),
            ("select minecraft:charge_type", 1),
            ("range minecraft:crossbow/pull", 1),
            ("condition minecraft:broken", 1),
            ("range minecraft:use_duration", 1),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_owned(), v))
        .collect();
        assert_eq!(properties, expected_properties);
        let expected_tints: BTreeMap<String, usize> = [
            ("minecraft:dye", 50),
            ("minecraft:constant", 11),
            ("minecraft:grass", 6),
            ("minecraft:potion", 4),
            ("minecraft:firework", 1),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_owned(), v))
        .collect();
        assert_eq!(tints, expected_tints);
    }

    fn parse(json: &str) -> Result<ClientItem, String> {
        ClientItem::parse(json.as_bytes())
    }

    fn model(json: &str) -> Result<ClientItem, String> {
        parse(&format!(r#"{{"model": {json}}}"#))
    }

    const LEAF: &str = r#"{"type": "minecraft:model", "model": "minecraft:item/stick"}"#;

    #[test]
    fn hand_written_shapes_the_corpus_lacks_parse_and_round_trip() {
        let cases = [
            format!(r#"{{"type": "minecraft:condition", "property": "minecraft:custom_model_data", "index": 2, "on_true": {LEAF}, "on_false": {LEAF}}}"#),
            format!(r#"{{"type": "minecraft:select", "property": "minecraft:custom_model_data", "index": 1, "cases": [{{"when": ["a", "b"], "model": {LEAF}}}], "fallback": {LEAF}}}"#),
            format!(r#"{{"type": "minecraft:range_dispatch", "property": "minecraft:custom_model_data", "index": 3, "scale": 2.0, "entries": [{{"threshold": 0.5, "model": {LEAF}}}]}}"#),
            r#"{"type": "minecraft:model", "model": "minecraft:item/stick", "tints": [{"type": "minecraft:custom_model_data", "index": 1, "default": -33024}, {"type": "minecraft:team", "default": 16777215}]}"#.to_string(),
            format!(r#"{{"type": "minecraft:select", "property": "minecraft:component", "cases": [{{"when": 3, "model": {LEAF}}}, {{"when": [4, 5], "model": {LEAF}}}], "component": "minecraft:max_stack_size"}}"#),
            format!(r#"{{"type": "minecraft:select", "property": "minecraft:component", "component": "minecraft:dyed_color", "cases": [{{"when": [1, 2], "model": {LEAF}}}]}}"#),
            format!(r#"{{"type": "minecraft:condition", "property": "minecraft:component", "predicate": "minecraft:damage", "value": {{"damage": {{"min": 1}}}}, "on_true": {LEAF}, "on_false": {LEAF}}}"#),
            format!(r#"{{"type": "minecraft:condition", "property": "minecraft:component", "value": {{"durability": 5}}, "predicate": "minecraft:damage", "on_true": {LEAF}, "on_false": {LEAF}}}"#),
            format!(r#"{{"type": "minecraft:range_dispatch", "property": "minecraft:damage", "normalize": false, "entries": [{{"threshold": 1, "model": {LEAF}}}]}}"#),
            format!(r#"{{"type": "minecraft:range_dispatch", "property": "minecraft:count", "entries": [{{"threshold": 1, "model": {LEAF}}}]}}"#),
            r#"{"type": "minecraft:range_dispatch", "property": "minecraft:cooldown", "entries": []}"#.to_string(),
            r#"{"type": "minecraft:model", "model": "minecraft:item/stick", "transformation": {"translation": [0, 1, 0], "left_rotation": [0, 0, 0, 1], "scale": [1, 1, 1], "right_rotation": {"angle": 0.5, "axis": [0, 1, 0]}}}"#.to_string(),
            r#"{"type": "minecraft:model", "model": "minecraft:item/stick", "transformation": [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1]}"#.to_string(),
            format!(r#"{{"type": "minecraft:select", "property": "minecraft:main_hand", "cases": [{{"when": "left", "model": {LEAF}}}]}}"#),
            format!(r#"{{"type": "minecraft:condition", "property": "minecraft:keybind_down", "keybind": "key.jump", "on_true": {LEAF}, "on_false": {LEAF}}}"#),
            r#"{"type": "minecraft:special", "base": "minecraft:item/shield", "model": {"type": "minecraft:head", "kind": "dragon", "animation": 0.5}}"#.to_string(),
            r#"{"type": "minecraft:special", "base": "minecraft:item/shield", "model": {"type": "minecraft:book", "open_angle": 1.0, "page1": 0.0, "page2": 0.5}}"#.to_string(),
            r#"{"type": "minecraft:special", "base": "minecraft:item/shield", "model": {"type": "minecraft:end_cube", "effect": "gateway"}}"#.to_string(),
        ];
        for json in &cases {
            let item = model(json).unwrap_or_else(|error| panic!("{json}: {error}"));
            let original: Value = serde_json::from_str(json).unwrap();
            let written = serde_json::to_value(&item.model).unwrap();
            assert!(structurally_equal(&original, &written), "{json}\n{written}");
        }
    }

    #[test]
    fn the_component_switch_reads_its_cases_with_the_components_codec() {
        use mcrs_minecraft_item_component::DyedColor;

        let item = model(&format!(
            r#"{{"type": "minecraft:select", "property": "minecraft:component", "component": "minecraft:dyed_color", "cases": [{{"when": [255, [1.0, 0.0, 0.0]], "model": {LEAF}}}]}}"#
        ))
        .unwrap();
        let UnbakedItemModel::Select {
            switch: SelectSwitch::Component(switch),
            ..
        } = &item.model
        else {
            panic!("{:?}", item.model);
        };
        assert_eq!(switch.component, ItemComponentKind::DyedColor);
        assert_eq!(
            switch.cases[0].when,
            [
                ItemComponentValue::DyedColor(DyedColor(RgbInt(255))),
                ItemComponentValue::DyedColor(DyedColor(RgbInt(-65536))),
            ]
        );
    }

    #[test]
    fn malformed_assets_are_refused_at_load() {
        let refused = [
            (format!(r#"{{"type": "minecraft:select", "property": "minecraft:charge_type", "cases": [{{"when": "arrow", "model": {LEAF}}}, {{"when": ["rocket", "arrow"], "model": {LEAF}}}]}}"#), "Duplicate case conditions"),
            (r#"{"type": "minecraft:select", "property": "minecraft:charge_type", "cases": []}"#.to_string(), "Empty case list"),
            (format!(r#"{{"type": "minecraft:select", "property": "minecraft:charge_type", "cases": [{{"when": [], "model": {LEAF}}}]}}"#), "List must have contents"),
            (format!(r#"{{"type": "minecraft:select", "property": "minecraft:component", "component": "minecraft:creative_slot_lock", "cases": [{{"when": {{}}, "model": {LEAF}}}]}}"#), "Component can't be serialized"),
            (format!(r#"{{"type": "minecraft:select", "property": "minecraft:charge_type", "cases": [{{"when": "arrow", "model": {LEAF}, "extra": 1}}]}}"#), "unknown field"),
            (format!(r#"{{"type": "minecraft:range_dispatch", "property": "minecraft:count", "entries": [{{"threshold": 1, "model": {LEAF}, "extra": 1}}]}}"#), "unknown field"),
            (r#"{"type": "minecraft:model", "model": "minecraft:item/stick", "tints": [{"type": "minecraft:dye", "default": 0, "extra": 1}]}"#.to_owned(), "unknown field"),
            (r#"{"type": "minecraft:model", "model": "minecraft:item/stick", "tints": [{"type": "minecraft:grass", "temperature": 2.0, "downfall": 0.5}]}"#.to_owned(), "outside [0, 1]"),
            (r#"{"type": "minecraft:special", "base": "minecraft:item/shield", "model": {"type": "minecraft:book", "open_angle": 1.0}}"#.to_owned(), "missing field"),
        ];
        for (json, message) in &refused {
            let error = model(json).expect_err(json);
            assert!(error.contains(message), "{json}: {error}");
        }
        let error = parse(&format!(r#"{{"model": {LEAF}, "extra": true}}"#)).unwrap_err();
        assert!(error.contains("unknown field"), "{error}");
    }

    /// `flatten` leaves the outer object without `deny_unknown_fields`.
    #[test]
    fn an_unknown_key_on_a_condition_object_passes_silently() {
        model(&format!(
            r#"{{"type": "minecraft:condition", "property": "minecraft:damaged", "on_true": {LEAF}, "on_false": {LEAF}, "extra": 1}}"#
        ))
        .unwrap();
    }
}
