use std::collections::BTreeMap;

use mcrs_minecraft_item::Text;
use mcrs_minecraft_item::component::banner::BannerPatterns;
use mcrs_minecraft_item::component::book::{TITLE_CHARS, WRITABLE_PAGE_CHARS};
use mcrs_minecraft_item::component::common::{
    BoundedString, EquipmentSlotGroup, Filterable, RgbInt,
};
use mcrs_minecraft_item::component::fireworks::FireworkExplosion;
use mcrs_minecraft_item::component::nbt_wire::CustomData;
use mcrs_minecraft_item::component::predicate::ItemPredicate;
use mcrs_minecraft_item::patch::ComponentPatch;
use mcrs_minecraft_loot_condition::{ConditionFile, FloatProviderFile, IntProviderFile};
use mcrs_minecraft_registries::Named;
use mcrs_minecraft_resource::ResourceLocationBuf;
use serde::Deserialize;

use crate::keyed_numbers::KeyedNumbers;
use crate::list_operation::{self, ListOperation, StandaloneList};
use crate::loot_table_file::{EntryFile, TagName};

/// An item modifier as a data pack writes it (REG-FLD-741): a tag or the key of registered
/// modifiers, a list applied in order, or one object.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum ModifierFile {
    Tag(TagName),
    Key(ResourceLocationBuf),
    Many(Vec<ModifierElement>),
    One(Box<ModifierObject>),
}

impl mcrs_minecraft_resource::RegistryValue for ModifierFile {
    type Registry = mcrs_minecraft_loot::function::LootItemFunction;
}

mcrs_minecraft_registries::resolve_as_is!(ModifierFile);

/// An element of a list of modifiers: a key or an object. A list is not an element (REG-FLD-780).
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum ModifierElement {
    Key(ResourceLocationBuf),
    Inline(Box<ModifierObject>),
}

/// The `functions` of a `sequence`: a tag, a list, or one element.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum ModifierSetFile {
    Tag(TagName),
    Many(Vec<ModifierElement>),
    One(ModifierElement),
}

#[derive(Debug, Clone, Deserialize)]
pub struct ModifierObject {
    pub condition: Option<ConditionFile>,
    #[serde(flatten)]
    pub kind: ModifierKind,
}

/// A set of registry entries: a tag, a list, or one key.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum KeySetFile {
    Tag(TagName),
    Many(Vec<ResourceLocationBuf>),
    One(ResourceLocationBuf),
}

/// The type of a modifier and its fields, REG-FLD-743 to REG-FLD-789; a type without a variant
/// fails the decode.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type")]
pub enum ModifierKind {
    #[serde(rename = "minecraft:set_count", alias = "set_count")]
    SetCount {
        count: IntProviderFile,
        #[serde(default)]
        add: bool,
    },
    #[serde(rename = "minecraft:set_item", alias = "set_item")]
    SetItem { item: ResourceLocationBuf },
    #[serde(
        rename = "minecraft:enchant_with_levels",
        alias = "enchant_with_levels"
    )]
    EnchantWithLevels {
        levels: IntProviderFile,
        options: Option<KeySetFile>,
        #[serde(default)]
        include_additional_cost_component: bool,
    },
    #[serde(rename = "minecraft:enchant_randomly", alias = "enchant_randomly")]
    EnchantRandomly {
        options: Option<KeySetFile>,
        #[serde(default = "yes")]
        only_compatible: bool,
        #[serde(default)]
        include_additional_cost_component: bool,
    },
    #[serde(rename = "minecraft:set_enchantments", alias = "set_enchantments")]
    SetEnchantments {
        #[serde(default)]
        enchantments: KeyedNumbers,
        #[serde(default)]
        add: bool,
    },
    #[serde(rename = "minecraft:set_custom_data", alias = "set_custom_data")]
    SetCustomData { tag: CustomData },
    #[serde(rename = "minecraft:set_components", alias = "set_components")]
    SetComponents { components: ComponentPatch<Named> },
    #[serde(rename = "minecraft:furnace_smelt", alias = "furnace_smelt")]
    FurnaceSmelt {
        #[serde(default = "yes")]
        use_input_count: bool,
    },
    #[serde(
        rename = "minecraft:enchanted_count_increase",
        alias = "enchanted_count_increase"
    )]
    EnchantedCountIncrease {
        enchantment: ResourceLocationBuf,
        count: FloatProviderFile,
        #[serde(default)]
        limit: i32,
    },
    #[serde(rename = "minecraft:set_damage", alias = "set_damage")]
    SetDamage {
        damage: FloatProviderFile,
        #[serde(default)]
        add: bool,
    },
    #[serde(rename = "minecraft:set_attributes", alias = "set_attributes")]
    SetAttributes {
        modifiers: Vec<AttributeFile>,
        #[serde(default = "yes")]
        replace: bool,
    },
    #[serde(rename = "minecraft:set_name", alias = "set_name")]
    SetName {
        name: Option<Text<Named>>,
        entity: Option<EntityTarget>,
        #[serde(default)]
        target: NameTarget,
    },
    #[serde(rename = "minecraft:exploration_map", alias = "exploration_map")]
    ExplorationMap {
        destination: KeySetFile,
        decoration: Option<ResourceLocationBuf>,
        #[serde(default = "two")]
        zoom: u8,
        #[serde(default = "fifty")]
        search_radius: i32,
        #[serde(default = "yes")]
        skip_existing_chunks: bool,
    },
    #[serde(rename = "minecraft:set_stew_effect", alias = "set_stew_effect")]
    SetStewEffect {
        #[serde(default)]
        effects: Vec<StewEffectFile>,
    },
    #[serde(rename = "minecraft:copy_name", alias = "copy_name")]
    CopyName { source: ContextTarget },
    #[serde(rename = "minecraft:set_contents", alias = "set_contents")]
    SetContents {
        component: ContentsComponent,
        entries: Vec<EntryFile>,
    },
    #[serde(rename = "minecraft:modify_contents", alias = "modify_contents")]
    ModifyContents {
        component: ContentsComponent,
        modifier: Box<ModifierFile>,
    },
    #[serde(rename = "minecraft:filtered", alias = "filtered")]
    Filtered {
        item_filter: Box<ItemPredicate<Named>>,
        on_pass: Option<Box<ModifierFile>>,
        on_fail: Option<Box<ModifierFile>>,
    },
    #[serde(rename = "minecraft:limit_count", alias = "limit_count")]
    LimitCount { limit: LimitFile },
    #[serde(rename = "minecraft:apply_bonus", alias = "apply_bonus")]
    ApplyBonus {
        enchantment: ResourceLocationBuf,
        #[serde(flatten)]
        formula: BonusFormula,
    },
    #[serde(rename = "minecraft:set_loot_table", alias = "set_loot_table")]
    SetLootTable {
        loot_table_id: ResourceLocationBuf,
        #[serde(default)]
        seed: i64,
    },
    #[serde(rename = "minecraft:explosion_decay", alias = "explosion_decay")]
    ExplosionDecay,
    #[serde(rename = "minecraft:discard", alias = "discard")]
    Discard,
    #[serde(rename = "minecraft:set_lore", alias = "set_lore")]
    SetLore {
        #[serde(deserialize_with = "list_operation::at_most::<_, _, 256>")]
        lore: Vec<Text<Named>>,
        entity: Option<EntityTarget>,
        #[serde(flatten)]
        operation: ListOperation,
    },
    #[serde(rename = "minecraft:fill_player_head", alias = "fill_player_head")]
    FillPlayerHead { entity: EntityTarget },
    #[serde(rename = "minecraft:copy_custom_data", alias = "copy_custom_data")]
    CopyCustomData {
        source: NbtSourceFile,
        ops: Vec<CopyOperationFile>,
    },
    #[serde(rename = "minecraft:copy_state", alias = "copy_state")]
    CopyState {
        block: ResourceLocationBuf,
        properties: Vec<String>,
    },
    #[serde(rename = "minecraft:set_banner_pattern", alias = "set_banner_pattern")]
    SetBannerPattern {
        patterns: BannerPatterns<Named>,
        append: bool,
    },
    #[serde(rename = "minecraft:set_potion", alias = "set_potion")]
    SetPotion { id: ResourceLocationBuf },
    #[serde(rename = "minecraft:set_random_dyes", alias = "set_random_dyes")]
    SetRandomDyes { number_of_dyes: IntProviderFile },
    #[serde(rename = "minecraft:set_random_potion", alias = "set_random_potion")]
    SetRandomPotion { options: Option<KeySetFile> },
    #[serde(rename = "minecraft:set_instrument", alias = "set_instrument")]
    SetInstrument { options: KeySetFile },
    #[serde(rename = "minecraft:sequence", alias = "sequence")]
    Sequence { functions: ModifierSetFile },
    #[serde(rename = "minecraft:copy_components", alias = "copy_components")]
    CopyComponents {
        source: CopySource,
        include: Option<Vec<ResourceLocationBuf>>,
        exclude: Option<Vec<ResourceLocationBuf>>,
    },
    #[serde(rename = "minecraft:set_fireworks", alias = "set_fireworks")]
    SetFireworks {
        flight_duration: Option<u8>,
        explosions: Option<StandaloneList<FireworkExplosion, 256>>,
    },
    #[serde(
        rename = "minecraft:set_firework_explosion",
        alias = "set_firework_explosion"
    )]
    SetFireworkExplosion {
        shape: Option<FireworkShape>,
        colors: Option<Vec<i32>>,
        fade_colors: Option<Vec<i32>>,
        trail: Option<bool>,
        twinkle: Option<bool>,
    },
    #[serde(rename = "minecraft:set_book_cover", alias = "set_book_cover")]
    SetBookCover {
        title: Option<Filterable<BoundedString<TITLE_CHARS>>>,
        author: Option<String>,
        generation: Option<Generation>,
    },
    #[serde(
        rename = "minecraft:set_written_book_pages",
        alias = "set_written_book_pages"
    )]
    SetWrittenBookPages {
        // chisle: the 32767 bound of a page (REG-FLD-561) is unchecked until the item crate exports it.
        pages: Vec<Filterable<Text<Named>>>,
        #[serde(flatten)]
        operation: ListOperation,
    },
    #[serde(
        rename = "minecraft:set_writable_book_pages",
        alias = "set_writable_book_pages"
    )]
    SetWritableBookPages {
        #[serde(deserialize_with = "list_operation::at_most::<_, _, 100>")]
        pages: Vec<Filterable<BoundedString<WRITABLE_PAGE_CHARS>>>,
        #[serde(flatten)]
        operation: ListOperation,
    },
    #[serde(rename = "minecraft:toggle_tooltips", alias = "toggle_tooltips")]
    ToggleTooltips { toggles: KeyedFlags },
    #[serde(
        rename = "minecraft:set_ominous_bottle_amplifier",
        alias = "set_ominous_bottle_amplifier"
    )]
    SetOminousBottleAmplifier { amplifier: IntProviderFile },
    #[serde(
        rename = "minecraft:set_custom_model_data",
        alias = "set_custom_model_data"
    )]
    SetCustomModelData {
        floats: Option<StandaloneList<FloatProviderFile>>,
        flags: Option<StandaloneList<bool>>,
        strings: Option<StandaloneList<String>>,
        colors: Option<StandaloneList<ColorFile>>,
    },
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum LimitFile {
    Exact(i32),
    Range {
        min: Option<IntProviderFile>,
        max: Option<IntProviderFile>,
    },
}

/// The `formula` of `apply_bonus` with its `parameters`, each of them required (REG-FLD-765).
/// An absent `parameters` reads as `{}`, which only `ore_drops` accepts.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(tag = "formula")]
pub enum BonusFormula {
    #[serde(rename = "minecraft:ore_drops", alias = "ore_drops")]
    OreDrops {
        #[serde(default)]
        parameters: NoParameters,
    },
    #[serde(
        rename = "minecraft:uniform_bonus_count",
        alias = "uniform_bonus_count"
    )]
    UniformBonusCount { parameters: UniformParameters },
    #[serde(
        rename = "minecraft:binomial_with_bonus_count",
        alias = "binomial_with_bonus_count"
    )]
    BinomialWithBonusCount { parameters: BinomialParameters },
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
pub struct NoParameters {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub struct UniformParameters {
    #[serde(rename = "bonusMultiplier")]
    pub bonus_multiplier: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
pub struct BinomialParameters {
    pub extra: i32,
    pub probability: f32,
}

/// An element of `colors.values` of `set_custom_model_data`: an int provider, or on read an RGB
/// color `[r, g, b]` (REG-FLD-789).
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum ColorFile {
    Provider(IntProviderFile),
    Rgb(RgbInt),
}

#[derive(Debug, Clone, Deserialize)]
pub struct AttributeFile {
    pub attribute: ResourceLocationBuf,
    pub id: ResourceLocationBuf,
    pub amount: FloatProviderFile,
    pub operation: AttributeOperation,
    pub slot: SlotsFile,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum AttributeOperation {
    #[serde(rename = "add_value")]
    Value,
    #[serde(rename = "add_multiplied_base")]
    MultipliedBase,
    #[serde(rename = "add_multiplied_total")]
    MultipliedTotal,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum SlotsFile {
    Many(#[serde(deserialize_with = "list_operation::not_empty")] Vec<EquipmentSlotGroup>),
    One(EquipmentSlotGroup),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntityTarget {
    This,
    Attacker,
    DirectAttacker,
    AttackingPlayer,
    TargetEntity,
    InteractingEntity,
}

/// The source of `copy_components`: an entity target, the block entity, or the tool
/// (REG-FLD-781).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CopySource {
    This,
    Attacker,
    DirectAttacker,
    AttackingPlayer,
    TargetEntity,
    InteractingEntity,
    BlockEntity,
    Tool,
}

/// The source of `copy_name` and the target of an NBT provider: an entity target or the block
/// entity (REG-FLD-759, REG-FLD-773).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextTarget {
    This,
    Attacker,
    DirectAttacker,
    AttackingPlayer,
    TargetEntity,
    InteractingEntity,
    BlockEntity,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NameTarget {
    #[default]
    CustomName,
    ItemName,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StewEffectFile {
    #[serde(rename = "type")]
    pub effect: ResourceLocationBuf,
    pub duration: IntProviderFile,
}

/// The component that `set_contents` and `modify_contents` write (REG-FLD-760).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum ContentsComponent {
    #[serde(rename = "minecraft:container", alias = "container")]
    Container,
    #[serde(rename = "minecraft:bundle_contents", alias = "bundle_contents")]
    BundleContents,
    #[serde(
        rename = "minecraft:charged_projectiles",
        alias = "charged_projectiles"
    )]
    ChargedProjectiles,
}

/// An NBT provider (REG-FLD-773).
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum NbtSourceFile {
    Target(ContextTarget),
    Typed(NbtSourceKind),
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type")]
pub enum NbtSourceKind {
    #[serde(rename = "minecraft:storage", alias = "storage")]
    Storage { source: ResourceLocationBuf },
    #[serde(rename = "minecraft:context", alias = "context")]
    Context { target: ContextTarget },
}

#[derive(Debug, Clone, Deserialize)]
pub struct CopyOperationFile {
    pub source: String,
    pub target: String,
    pub op: CopyOp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CopyOp {
    Replace,
    Append,
    Merge,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FireworkShape {
    SmallBall,
    LargeBall,
    Star,
    Creeper,
    Burst,
}

/// The `toggles` of `toggle_tooltips`: a flag for each component type.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(transparent)]
pub struct KeyedFlags(pub BTreeMap<ResourceLocationBuf, bool>);

/// The `generation` of `set_book_cover`: from 0 to 3 (REG-FLD-784).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(try_from = "u8")]
pub struct Generation(pub u8);

impl TryFrom<u8> for Generation {
    type Error = &'static str;

    fn try_from(generation: u8) -> Result<Self, Self::Error> {
        match generation <= 3 {
            true => Ok(Self(generation)),
            false => Err("a generation above 3"),
        }
    }
}

fn yes() -> bool {
    true
}

fn two() -> u8 {
    2
}

fn fifty() -> i32 {
    50
}
