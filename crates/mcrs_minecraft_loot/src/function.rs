use std::fmt;

use mcrs_minecraft_block::keys::Block;
use mcrs_minecraft_core::codec::{
    BoundedString, CompactList, NonNegativeInt, Validate, default_true, is_true,
};
use mcrs_minecraft_core::registry_key::RegistryValue;
use mcrs_minecraft_core::{ResourceKey, ResourceLocation, validated};
use mcrs_minecraft_entity::keys::Attribute;
use mcrs_minecraft_item::component::banner::BannerPatterns;
use mcrs_minecraft_item::component::common::{EquipmentSlotGroup, Filterable, NbtPredicate};
use mcrs_minecraft_item::component::fireworks::{FireworkExplosion, FireworkShape};
use mcrs_minecraft_item::component::predicate::ItemPredicate;
use mcrs_minecraft_item::enchantment::EnchantmentData;
use mcrs_minecraft_item::keys::{DataComponentType, Item, MapDecorationType, MobEffect, Potion};
use mcrs_minecraft_item::loot::LootTable;
use mcrs_minecraft_item::patch::ComponentPatch;
use mcrs_minecraft_item::{AttributeOperation, InstrumentValue, Text};
use mcrs_minecraft_registry::{Holder, HolderList, HolderSet, Id};
use mcrs_minecraft_worldgen_structure::Structure;
use serde::de::{Error as _, MapAccess, SeqAccess, Visitor, value};
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::condition::{EntityTarget, LootCondition};
use crate::entry::LootPoolEntry;
use crate::number::{FloatExpression, IntExpression};
use crate::provider::{ComponentSource, EntityOrBlock, NbtProvider};
use crate::slot::ContainerComponent;

/// A change to an item stack, registered in `item_modifier` or written inline;
/// a list written inline applies each in turn.
#[derive(Debug, Clone, PartialEq)]
pub enum LootItemFunction {
    Sequence(HolderList<LootItemFunction>),
    Typed(Box<TypedFunction>),
}

impl RegistryValue for LootItemFunction {
    type Registry = Self;
}

impl Serialize for LootItemFunction {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            LootItemFunction::Sequence(functions) => functions.serialize(s),
            LootItemFunction::Typed(typed) => typed.serialize(s),
        }
    }
}

impl<'de> Deserialize<'de> for LootItemFunction {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct FunctionVisitor;

        impl<'de> Visitor<'de> for FunctionVisitor {
            type Value = LootItemFunction;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("an item modifier or a list of them")
            }

            fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<LootItemFunction, E> {
                HolderList::deserialize(value::StrDeserializer::new(v))
                    .map(LootItemFunction::Sequence)
            }

            fn visit_seq<A: SeqAccess<'de>>(self, seq: A) -> Result<LootItemFunction, A::Error> {
                HolderList::deserialize(value::SeqAccessDeserializer::new(seq))
                    .map(LootItemFunction::Sequence)
            }

            fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<LootItemFunction, A::Error> {
                match TypedFunction::deserialize(value::MapAccessDeserializer::new(map))? {
                    TypedFunction::Sequence(Sequence {
                        condition: None,
                        functions,
                    }) => Ok(LootItemFunction::Sequence(functions)),
                    typed => Ok(LootItemFunction::Typed(Box::new(typed))),
                }
            }
        }

        d.deserialize_any(FunctionVisitor)
    }
}

/// Fields every typed modifier carries: the condition under which it applies.
macro_rules! function {
    ($(#[$meta:meta])* pub struct $name:ident { $($(#[$fmeta:meta])* pub $field:ident : $ty:ty,)* }) => {
        #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
        #[serde(deny_unknown_fields)]
        $(#[$meta])*
        pub struct $name {
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub condition: Option<Holder<LootCondition>>,
            $($(#[$fmeta])* pub $field: $ty,)*
        }
    };
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum TypedFunction {
    #[serde(rename = "minecraft:set_count", alias = "set_count")]
    SetCount(SetCount),
    #[serde(rename = "minecraft:set_item", alias = "set_item")]
    SetItem(SetItem),
    #[serde(
        rename = "minecraft:enchant_with_levels",
        alias = "enchant_with_levels"
    )]
    EnchantWithLevels(EnchantWithLevels),
    #[serde(rename = "minecraft:enchant_randomly", alias = "enchant_randomly")]
    EnchantRandomly(EnchantRandomly),
    #[serde(rename = "minecraft:set_enchantments", alias = "set_enchantments")]
    SetEnchantments(SetEnchantments),
    #[serde(rename = "minecraft:set_custom_data", alias = "set_custom_data")]
    SetCustomData(SetCustomData),
    #[serde(rename = "minecraft:set_components", alias = "set_components")]
    SetComponents(SetComponents),
    #[serde(rename = "minecraft:furnace_smelt", alias = "furnace_smelt")]
    FurnaceSmelt(FurnaceSmelt),
    #[serde(
        rename = "minecraft:enchanted_count_increase",
        alias = "enchanted_count_increase"
    )]
    EnchantedCountIncrease(EnchantedCountIncrease),
    #[serde(rename = "minecraft:set_damage", alias = "set_damage")]
    SetDamage(SetDamage),
    #[serde(rename = "minecraft:set_attributes", alias = "set_attributes")]
    SetAttributes(SetAttributes),
    #[serde(rename = "minecraft:set_name", alias = "set_name")]
    SetName(SetName),
    #[serde(rename = "minecraft:exploration_map", alias = "exploration_map")]
    ExplorationMap(ExplorationMap),
    #[serde(rename = "minecraft:set_stew_effect", alias = "set_stew_effect")]
    SetStewEffect(SetStewEffect),
    #[serde(rename = "minecraft:copy_name", alias = "copy_name")]
    CopyName(CopyName),
    #[serde(rename = "minecraft:set_contents", alias = "set_contents")]
    SetContents(SetContents),
    #[serde(rename = "minecraft:modify_contents", alias = "modify_contents")]
    ModifyContents(ModifyContents),
    #[serde(rename = "minecraft:filtered", alias = "filtered")]
    Filtered(Filtered),
    #[serde(rename = "minecraft:limit_count", alias = "limit_count")]
    LimitCount(LimitCount),
    #[serde(rename = "minecraft:apply_bonus", alias = "apply_bonus")]
    ApplyBonus(ApplyBonus),
    #[serde(rename = "minecraft:set_loot_table", alias = "set_loot_table")]
    SetLootTable(SetLootTable),
    #[serde(rename = "minecraft:explosion_decay", alias = "explosion_decay")]
    ExplosionDecay(Conditioned),
    #[serde(rename = "minecraft:set_lore", alias = "set_lore")]
    SetLore(SetLore),
    #[serde(rename = "minecraft:fill_player_head", alias = "fill_player_head")]
    FillPlayerHead(FillPlayerHead),
    #[serde(rename = "minecraft:copy_custom_data", alias = "copy_custom_data")]
    CopyCustomData(CopyCustomData),
    #[serde(rename = "minecraft:copy_state", alias = "copy_state")]
    CopyState(CopyState),
    #[serde(rename = "minecraft:set_banner_pattern", alias = "set_banner_pattern")]
    SetBannerPattern(SetBannerPattern),
    #[serde(rename = "minecraft:set_potion", alias = "set_potion")]
    SetPotion(SetPotion),
    #[serde(rename = "minecraft:set_random_dyes", alias = "set_random_dyes")]
    SetRandomDyes(SetRandomDyes),
    #[serde(rename = "minecraft:set_random_potion", alias = "set_random_potion")]
    SetRandomPotion(SetRandomPotion),
    #[serde(rename = "minecraft:set_instrument", alias = "set_instrument")]
    SetInstrument(SetInstrument),
    #[serde(rename = "minecraft:sequence", alias = "sequence")]
    Sequence(Sequence),
    #[serde(rename = "minecraft:copy_components", alias = "copy_components")]
    CopyComponents(CopyComponents),
    #[serde(rename = "minecraft:set_fireworks", alias = "set_fireworks")]
    SetFireworks(SetFireworks),
    #[serde(
        rename = "minecraft:set_firework_explosion",
        alias = "set_firework_explosion"
    )]
    SetFireworkExplosion(SetFireworkExplosion),
    #[serde(rename = "minecraft:set_book_cover", alias = "set_book_cover")]
    SetBookCover(SetBookCover),
    #[serde(
        rename = "minecraft:set_written_book_pages",
        alias = "set_written_book_pages"
    )]
    SetWrittenBookPages(SetWrittenBookPages),
    #[serde(
        rename = "minecraft:set_writable_book_pages",
        alias = "set_writable_book_pages"
    )]
    SetWritableBookPages(SetWritableBookPages),
    #[serde(rename = "minecraft:toggle_tooltips", alias = "toggle_tooltips")]
    ToggleTooltips(ToggleTooltips),
    #[serde(
        rename = "minecraft:set_ominous_bottle_amplifier",
        alias = "set_ominous_bottle_amplifier"
    )]
    SetOminousBottleAmplifier(SetOminousBottleAmplifier),
    #[serde(
        rename = "minecraft:set_custom_model_data",
        alias = "set_custom_model_data"
    )]
    SetCustomModelData(SetCustomModelData),
    #[serde(rename = "minecraft:discard", alias = "discard")]
    Discard(Conditioned),
}

function! {
    /// A modifier with nothing to state beyond its condition.
    pub struct Conditioned {}
}

function! {
    pub struct SetCount {
        pub count: Holder<IntExpression>,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        pub add: bool,
    }
}

function! {
    pub struct SetItem {
        pub item: Item,
    }
}

function! {
    pub struct EnchantWithLevels {
        pub levels: Holder<IntExpression>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub options: Option<HolderSet<EnchantmentData>>,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        pub include_additional_cost_component: bool,
    }
}

function! {
    pub struct EnchantRandomly {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub options: Option<HolderSet<EnchantmentData>>,
        #[serde(default = "default_true", skip_serializing_if = "is_true")]
        pub only_compatible: bool,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        pub include_additional_cost_component: bool,
    }
}

function! {
    pub struct SetEnchantments {
        #[serde(default, skip_serializing_if = "Enchantments::is_empty")]
        pub enchantments: Enchantments,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        pub add: bool,
    }
}

/// Levels by enchantment, in the order read.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Enchantments(pub Vec<(Id<EnchantmentData>, Holder<IntExpression>)>);

impl Enchantments {
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl Serialize for Enchantments {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut map = s.serialize_map(Some(self.0.len()))?;
        for (enchantment, level) in &self.0 {
            map.serialize_entry(enchantment, level)?;
        }
        map.end()
    }
}

impl<'de> Deserialize<'de> for Enchantments {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct EnchantmentsVisitor;

        impl<'de> Visitor<'de> for EnchantmentsVisitor {
            type Value = Enchantments;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a map of enchantments to levels")
            }

            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Enchantments, A::Error> {
                let mut entries: Vec<(Id<EnchantmentData>, Holder<IntExpression>)> = Vec::new();
                while let Some(enchantment) = map.next_key::<Id<EnchantmentData>>()? {
                    if entries.iter().any(|(seen, _)| *seen == enchantment) {
                        return Err(A::Error::custom("Duplicate enchantment"));
                    }
                    entries.push((enchantment, map.next_value()?));
                }
                Ok(Enchantments(entries))
            }
        }

        d.deserialize_map(EnchantmentsVisitor)
    }
}

function! {
    pub struct SetCustomData {
        pub tag: NbtPredicate,
    }
}

function! {
    pub struct SetComponents {
        pub components: ComponentPatch,
    }
}

function! {
    pub struct FurnaceSmelt {
        #[serde(default = "default_true", skip_serializing_if = "is_true")]
        pub use_input_count: bool,
    }
}

function! {
    pub struct EnchantedCountIncrease {
        pub enchantment: Id<EnchantmentData>,
        pub count: Holder<FloatExpression>,
        #[serde(default, skip_serializing_if = "is_zero")]
        pub limit: i32,
    }
}

fn is_zero(value: &i32) -> bool {
    *value == 0
}

function! {
    pub struct SetDamage {
        pub damage: Holder<FloatExpression>,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        pub add: bool,
    }
}

function! {
    pub struct SetAttributes {
        pub modifiers: Vec<AttributeModifierSpec>,
        #[serde(default = "default_true", skip_serializing_if = "is_true")]
        pub replace: bool,
    }
}

validated!(AttributeModifierSpec);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(remote = "Self", deny_unknown_fields)]
pub struct AttributeModifierSpec {
    pub id: ResourceLocation,
    pub attribute: Attribute,
    pub operation: AttributeOperation,
    pub amount: Holder<FloatExpression>,
    pub slot: CompactList<EquipmentSlotGroup>,
}

impl Validate for AttributeModifierSpec {
    fn validate(&self) -> Result<(), String> {
        if self.slot.0.is_empty() {
            Err("List must have contents".into())
        } else {
            Ok(())
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NameTarget {
    #[default]
    CustomName,
    ItemName,
}

function! {
    pub struct SetName {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub name: Option<Text>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub entity: Option<EntityTarget>,
        #[serde(default, skip_serializing_if = "mcrs_minecraft_core::codec::is_default")]
        pub target: NameTarget,
    }
}

function! {
    pub struct ExplorationMap {
        pub destination: HolderSet<Structure>,
        #[serde(
            default = "default_decoration",
            skip_serializing_if = "is_default_decoration"
        )]
        pub decoration: MapDecorationType,
        #[serde(default = "default_zoom", skip_serializing_if = "is_default_zoom")]
        pub zoom: i8,
        #[serde(
            default = "default_search_radius",
            skip_serializing_if = "is_default_search_radius"
        )]
        pub search_radius: i32,
        #[serde(default = "default_true", skip_serializing_if = "is_true")]
        pub skip_existing_chunks: bool,
    }
}

fn default_decoration() -> MapDecorationType {
    MapDecorationType::Mansion
}

fn is_default_decoration(decoration: &MapDecorationType) -> bool {
    *decoration == default_decoration()
}

fn default_zoom() -> i8 {
    2
}

fn is_default_zoom(zoom: &i8) -> bool {
    *zoom == 2
}

fn default_search_radius() -> i32 {
    50
}

fn is_default_search_radius(radius: &i32) -> bool {
    *radius == 50
}

validated!(SetStewEffect);

function! {
    #[serde(remote = "Self")]
    pub struct SetStewEffect {
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        pub effects: Vec<StewEffect>,
    }
}

impl Validate for SetStewEffect {
    fn validate(&self) -> Result<(), String> {
        for (index, effect) in self.effects.iter().enumerate() {
            if self.effects[..index]
                .iter()
                .any(|seen| seen.effect == effect.effect)
            {
                return Err(format!(
                    "Encountered duplicate mob effect: '{}'",
                    effect.effect.as_static_str()
                ));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StewEffect {
    #[serde(rename = "type")]
    pub effect: MobEffect,
    pub duration: Holder<IntExpression>,
}

function! {
    pub struct CopyName {
        pub source: EntityOrBlock,
    }
}

function! {
    pub struct SetContents {
        pub component: ContainerComponent,
        pub entries: Vec<LootPoolEntry>,
    }
}

function! {
    pub struct ModifyContents {
        pub component: ContainerComponent,
        pub modifier: Holder<LootItemFunction>,
    }
}

function! {
    pub struct Filtered {
        pub item_filter: ItemPredicate,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub on_pass: Option<Holder<LootItemFunction>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub on_fail: Option<Holder<LootItemFunction>>,
    }
}

function! {
    pub struct LimitCount {
        pub limit: IntLimit,
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IntLimit {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min: Option<Holder<IntExpression>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<Holder<IntExpression>>,
}

/// How a bonus count grows with the enchantment's level, named by `formula`
/// and parameterised by `parameters` where the formula takes any.
#[derive(Debug, Clone, PartialEq)]
pub enum BonusFormula {
    BinomialWithBonusCount { extra: i32, probability: f32 },
    OreDrops,
    UniformBonusCount { bonus_multiplier: i32 },
}

#[derive(Debug, Clone, PartialEq)]
pub struct ApplyBonus {
    pub condition: Option<Holder<LootCondition>>,
    pub enchantment: Id<EnchantmentData>,
    pub formula: BonusFormula,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct BinomialParameters {
    extra: i32,
    probability: f32,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct UniformParameters {
    #[serde(rename = "bonusMultiplier")]
    bonus_multiplier: i32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ApplyBonusRepr {
    #[serde(default)]
    condition: Option<Holder<LootCondition>>,
    enchantment: Id<EnchantmentData>,
    formula: ResourceLocation,
    #[serde(default)]
    parameters: Option<crate::buffer::Buffered>,
}

impl Serialize for ApplyBonus {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut map = s.serialize_map(None)?;
        if let Some(condition) = &self.condition {
            map.serialize_entry("condition", condition)?;
        }
        map.serialize_entry("enchantment", &self.enchantment)?;
        match &self.formula {
            BonusFormula::BinomialWithBonusCount { extra, probability } => {
                map.serialize_entry("formula", "minecraft:binomial_with_bonus_count")?;
                map.serialize_entry(
                    "parameters",
                    &BinomialParameters {
                        extra: *extra,
                        probability: *probability,
                    },
                )?;
            }
            BonusFormula::OreDrops => map.serialize_entry("formula", "minecraft:ore_drops")?,
            BonusFormula::UniformBonusCount { bonus_multiplier } => {
                map.serialize_entry("formula", "minecraft:uniform_bonus_count")?;
                map.serialize_entry(
                    "parameters",
                    &UniformParameters {
                        bonus_multiplier: *bonus_multiplier,
                    },
                )?;
            }
        }
        map.end()
    }
}

impl<'de> Deserialize<'de> for ApplyBonus {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        use serde::de::IntoDeserializer as _;
        let repr = ApplyBonusRepr::deserialize(d)?;
        let parameters = |name: &str| {
            repr.parameters
                .clone()
                .ok_or_else(|| D::Error::custom(format_args!("{name} takes parameters")))
        };
        let formula = match repr.formula.as_str() {
            "minecraft:binomial_with_bonus_count" => {
                let p = BinomialParameters::deserialize(
                    parameters("binomial_with_bonus_count")?.into_deserializer(),
                )?;
                BonusFormula::BinomialWithBonusCount {
                    extra: p.extra,
                    probability: p.probability,
                }
            }
            "minecraft:ore_drops" => BonusFormula::OreDrops,
            "minecraft:uniform_bonus_count" => {
                let p = UniformParameters::deserialize(
                    parameters("uniform_bonus_count")?.into_deserializer(),
                )?;
                BonusFormula::UniformBonusCount {
                    bonus_multiplier: p.bonus_multiplier,
                }
            }
            other => {
                return Err(D::Error::custom(format_args!(
                    "No formula type with id: '{other}'"
                )));
            }
        };
        Ok(ApplyBonus {
            condition: repr.condition,
            enchantment: repr.enchantment,
            formula,
        })
    }
}

function! {
    pub struct SetLootTable {
        pub loot_table_id: ResourceKey<LootTable>,
        #[serde(default, skip_serializing_if = "is_zero_long")]
        pub seed: i64,
    }
}

fn is_zero_long(value: &i64) -> bool {
    *value == 0
}

function! {
    pub struct SetLore {
        pub lore: Vec<Text>,
        #[serde(flatten)]
        pub mode: ListOperation,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub entity: Option<EntityTarget>,
    }
}

function! {
    pub struct FillPlayerHead {
        pub entity: EntityTarget,
    }
}

function! {
    pub struct CopyCustomData {
        pub source: NbtProvider,
        pub ops: Vec<CopyOperation>,
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CopyOperation {
    // chisle: NBT paths are kept as written and not parsed, so a malformed path passes the load; parsing them with the NBT path grammar lifts this
    pub source: String,
    pub target: String,
    pub op: MergeStrategy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MergeStrategy {
    Replace,
    Append,
    Merge,
}

function! {
    pub struct CopyState {
        pub block: Block,
        pub properties: Vec<String>,
    }
}

function! {
    pub struct SetBannerPattern {
        pub patterns: BannerPatterns,
        pub append: bool,
    }
}

function! {
    pub struct SetPotion {
        pub id: Potion,
    }
}

function! {
    pub struct SetRandomDyes {
        pub number_of_dyes: Holder<IntExpression>,
    }
}

function! {
    pub struct SetRandomPotion {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub options: Option<HolderSet<Potion>>,
    }
}

function! {
    pub struct SetInstrument {
        pub options: HolderSet<InstrumentValue>,
    }
}

function! {
    pub struct Sequence {
        pub functions: HolderList<LootItemFunction>,
    }
}

function! {
    pub struct CopyComponents {
        pub source: ComponentSource,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub include: Option<Vec<DataComponentType>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub exclude: Option<Vec<DataComponentType>>,
    }
}

function! {
    pub struct SetFireworks {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub explosions: Option<StandAlone<FireworkExplosion>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub flight_duration: Option<u8>,
    }
}

function! {
    pub struct SetFireworkExplosion {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub shape: Option<FireworkShape>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub colors: Option<Vec<i32>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub fade_colors: Option<Vec<i32>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub trail: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub twinkle: Option<bool>,
    }
}

function! {
    pub struct SetBookCover {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub title: Option<Filterable<BoundedString<32>>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub author: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub generation: Option<mcrs_minecraft_core::codec::Bounded<0, 3>>,
    }
}

function! {
    pub struct SetWrittenBookPages {
        pub pages: Vec<Filterable<Text>>,
        #[serde(flatten)]
        pub mode: ListOperation,
    }
}

function! {
    pub struct SetWritableBookPages {
        pub pages: Vec<Filterable<BoundedString<1024>>>,
        #[serde(flatten)]
        pub mode: ListOperation,
    }
}

function! {
    pub struct ToggleTooltips {
        pub toggles: Toggles,
    }
}

/// Whether each component shows in the tooltip, in the order read.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Toggles(pub Vec<(DataComponentType, bool)>);

impl Serialize for Toggles {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut map = s.serialize_map(Some(self.0.len()))?;
        for (component, shown) in &self.0 {
            map.serialize_entry(component, shown)?;
        }
        map.end()
    }
}

impl<'de> Deserialize<'de> for Toggles {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct TogglesVisitor;

        impl<'de> Visitor<'de> for TogglesVisitor {
            type Value = Toggles;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a map of components to booleans")
            }

            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Toggles, A::Error> {
                let mut entries: Vec<(DataComponentType, bool)> = Vec::new();
                while let Some(component) = map.next_key::<DataComponentType>()? {
                    if entries.iter().any(|(seen, _)| *seen == component) {
                        return Err(A::Error::custom(format_args!(
                            "Duplicate key '{}'",
                            component.as_static_str()
                        )));
                    }
                    entries.push((component, map.next_value()?));
                }
                Ok(Toggles(entries))
            }
        }

        d.deserialize_map(TogglesVisitor)
    }
}

function! {
    pub struct SetOminousBottleAmplifier {
        pub amplifier: Holder<IntExpression>,
    }
}

function! {
    pub struct SetCustomModelData {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub floats: Option<StandAlone<Holder<FloatExpression>>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub flags: Option<StandAlone<bool>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub strings: Option<StandAlone<String>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub colors: Option<StandAlone<ColorProvider>>,
    }
}

/// An int provider for a colour, or the colour as `[r, g, b]` floats, which
/// reads as that packed constant.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(transparent)]
pub struct ColorProvider(pub Holder<IntExpression>);

impl<'de> Deserialize<'de> for ColorProvider {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct ColorVisitor;

        impl<'de> Visitor<'de> for ColorVisitor {
            type Value = ColorProvider;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("an int provider or an [r, g, b] colour")
            }

            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<ColorProvider, A::Error> {
                let mut channels = Vec::with_capacity(3);
                while let Some(channel) = seq.next_element::<f32>()? {
                    channels.push(channel);
                }
                let [r, g, b] = channels[..] else {
                    return Err(A::Error::invalid_length(channels.len(), &"3 channels"));
                };
                let pack = |c: f32| (c * 255.0).floor() as i32 & 0xFF;
                Ok(ColorProvider(Holder::Direct(IntExpression::Constant(
                    (pack(r) << 16) | (pack(g) << 8) | pack(b),
                ))))
            }

            fn visit_i64<E: serde::de::Error>(self, v: i64) -> Result<ColorProvider, E> {
                Holder::deserialize(value::I64Deserializer::new(v)).map(ColorProvider)
            }

            fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<ColorProvider, E> {
                Holder::deserialize(value::U64Deserializer::new(v)).map(ColorProvider)
            }

            fn visit_f64<E: serde::de::Error>(self, v: f64) -> Result<ColorProvider, E> {
                Holder::deserialize(value::F64Deserializer::new(v)).map(ColorProvider)
            }

            fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<ColorProvider, E> {
                Holder::deserialize(value::StrDeserializer::new(v)).map(ColorProvider)
            }

            fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<ColorProvider, A::Error> {
                Holder::deserialize(value::MapAccessDeserializer::new(map)).map(ColorProvider)
            }
        }

        d.deserialize_any(ColorVisitor)
    }
}

/// How a list modifier merges its values into the item's list: by `mode`,
/// with `offset` and `size` where the mode takes them.
#[derive(Debug, Clone, PartialEq)]
pub enum ListOperation {
    ReplaceAll,
    ReplaceSection { offset: i32, size: Option<i32> },
    Insert { offset: i32 },
    Append,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ListMode {
    ReplaceAll,
    ReplaceSection,
    Insert,
    Append,
}

#[derive(Serialize, Deserialize)]
struct ListOperationRepr {
    mode: ListMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    offset: Option<NonNegativeInt>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    size: Option<NonNegativeInt>,
}

impl Serialize for ListOperation {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let non_zero =
            |offset: i32| (offset != 0).then_some(mcrs_minecraft_core::codec::Bounded(offset));
        let repr = match self {
            ListOperation::ReplaceAll => ListOperationRepr {
                mode: ListMode::ReplaceAll,
                offset: None,
                size: None,
            },
            ListOperation::ReplaceSection { offset, size } => ListOperationRepr {
                mode: ListMode::ReplaceSection,
                offset: non_zero(*offset),
                size: size.map(mcrs_minecraft_core::codec::Bounded),
            },
            ListOperation::Insert { offset } => ListOperationRepr {
                mode: ListMode::Insert,
                offset: non_zero(*offset),
                size: None,
            },
            ListOperation::Append => ListOperationRepr {
                mode: ListMode::Append,
                offset: None,
                size: None,
            },
        };
        repr.serialize(s)
    }
}

impl<'de> Deserialize<'de> for ListOperation {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let repr = ListOperationRepr::deserialize(d)?;
        let offset = repr.offset.map_or(0, |offset| offset.0);
        let refuse = |field: &str| {
            Err(D::Error::custom(format_args!(
                "{field} does not apply to this list operation mode"
            )))
        };
        match repr.mode {
            ListMode::ReplaceAll if repr.offset.is_some() => refuse("offset"),
            ListMode::Append if repr.offset.is_some() => refuse("offset"),
            ListMode::ReplaceAll | ListMode::Append | ListMode::Insert if repr.size.is_some() => {
                refuse("size")
            }
            ListMode::ReplaceAll => Ok(ListOperation::ReplaceAll),
            ListMode::Append => Ok(ListOperation::Append),
            ListMode::Insert => Ok(ListOperation::Insert { offset }),
            ListMode::ReplaceSection => Ok(ListOperation::ReplaceSection {
                offset,
                size: repr.size.map(|size| size.0),
            }),
        }
    }
}

/// A list of values and how they merge into the item's list.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(bound(serialize = "T: Serialize", deserialize = "T: Deserialize<'de>"))]
pub struct StandAlone<T> {
    pub values: Vec<T>,
    #[serde(flatten)]
    pub operation: ListOperation,
}
