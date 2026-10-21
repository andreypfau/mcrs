mod ingredient;
mod pattern;

use std::fmt;
use std::ops::RangeInclusive;

use crate::Template;
use crate::component::common::MinMaxBounds;
use crate::component::predicate::PotionsPredicate;
use crate::component::trim::TrimPatternRef;
use crate::keys::Item;
use crate::patch::ComponentPatch;
use mcrs_minecraft_core::codec::{self, Validate, default_true, is_default, is_true};
use mcrs_minecraft_core::registry_key::RegistryValue;
use mcrs_minecraft_core::validated;
use mcrs_minecraft_registries::{Key, Named, Refs, Resolved, registry_value};
use serde::de::{MapAccess, Visitor, value};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

pub use ingredient::Ingredient;
use pattern::check_pattern;
pub use pattern::{Pattern, PatternKey};

/// A recipe as its data pack file states it, dispatched on its serializer.
#[registry_value]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(remote = "Self")]
pub enum Recipe<M: Refs = Resolved> {
    CraftingShaped(ShapedRecipe<M>),
    CraftingShapeless(ShapelessRecipe<M>),
    CraftingDye(DyeRecipe<M>),
    CraftingImbue(ImbueRecipe<M>),
    CraftingTransmute(TransmuteRecipe<M>),
    CraftingDecoratedPot(DecoratedPotRecipe<M>),
    BookCloning(BookCloningRecipe<M>),
    MapExtending(MapExtendingRecipe<M>),
    FireworkRocket(FireworkRocketRecipe<M>),
    FireworkStar(FireworkStarRecipe<M>),
    FireworkStarFade(FireworkStarFadeRecipe<M>),
    BannerDuplicate(BannerDuplicateRecipe<M>),
    ShieldDecoration(ShieldDecorationRecipe<M>),
    RepairItem,
    Smelting(CookingRecipe<M>),
    Blasting(CookingRecipe<M>),
    Smoking(CookingRecipe<M>),
    CampfireCooking(CookingRecipe<M>),
    Stonecutting(StonecutterRecipe<M>),
    SmithingTransform(SmithingTransformRecipe<M>),
    SmithingTrim(SmithingTrimRecipe<M>),
    Brewing(BrewingRecipe<M>),
}

mcrs_minecraft_registry::dispatch! {
    Recipe<Named>, key = "type", registry = crate::keys::RecipeSerializer,
    {
        CraftingShaped => CraftingShaped,
        CraftingShapeless => CraftingShapeless,
        CraftingDye => CraftingDye,
        CraftingImbue => CraftingImbue,
        CraftingTransmute => CraftingTransmute,
        CraftingDecoratedPot => CraftingDecoratedPot,
        CraftingSpecialBookcloning => BookCloning,
        CraftingSpecialMapextending => MapExtending,
        CraftingSpecialFireworkRocket => FireworkRocket,
        CraftingSpecialFireworkStar => FireworkStar,
        CraftingSpecialFireworkStarFade => FireworkStarFade,
        CraftingSpecialBannerduplicate => BannerDuplicate,
        CraftingSpecialShielddecoration => ShieldDecoration,
        CraftingSpecialRepairitem => RepairItem,
        Smelting => Smelting,
        Blasting => Blasting,
        Smoking => Smoking,
        CampfireCooking => CampfireCooking,
        Stonecutting => Stonecutting,
        SmithingTransform => SmithingTransform,
        Brewing => Brewing,
        SmithingTrim => SmithingTrim,
    }
}

impl RegistryValue for Recipe {
    type Registry = Self;
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CraftingBookCategory {
    Building,
    Redstone,
    Equipment,
    #[default]
    Misc,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CookingBookCategory {
    Food,
    Blocks,
    #[default]
    Misc,
}

validated!(ShapedRecipe<Named>);

#[registry_value]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(remote = "Self", deny_unknown_fields)]
pub struct ShapedRecipe<M: Refs = Resolved> {
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub show_notification: bool,
    #[serde(default, skip_serializing_if = "is_default")]
    pub category: CraftingBookCategory,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub group: String,
    pub key: PatternKey<M>,
    pub pattern: Pattern,
    pub result: Template<M>,
}

impl<M: Refs> Validate for ShapedRecipe<M> {
    fn validate(&self) -> Result<(), String> {
        check_pattern(&self.key, &self.pattern)
    }
}

#[registry_value]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShapelessRecipe<M: Refs = Resolved> {
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub show_notification: bool,
    #[serde(default, skip_serializing_if = "is_default")]
    pub category: CraftingBookCategory,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub group: String,
    pub result: Template<M>,
    #[serde(
        deserialize_with = "codec::sized_list::<1, 9, _, _>",
        bound(deserialize = "Ingredient<M>: Deserialize<'de>")
    )]
    pub ingredients: Vec<Ingredient<M>>,
}

#[registry_value]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DyeRecipe<M: Refs = Resolved> {
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub show_notification: bool,
    #[serde(default, skip_serializing_if = "is_default")]
    pub category: CraftingBookCategory,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub group: String,
    pub target: Ingredient<M>,
    pub dye: Ingredient<M>,
    pub result: Template<M>,
}

#[registry_value]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImbueRecipe<M: Refs = Resolved> {
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub show_notification: bool,
    #[serde(default, skip_serializing_if = "is_default")]
    pub category: CraftingBookCategory,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub group: String,
    pub source: Ingredient<M>,
    pub material: Ingredient<M>,
    pub result: Template<M>,
}

validated!(TransmuteRecipe<Named>);

#[registry_value]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(remote = "Self", deny_unknown_fields)]
pub struct TransmuteRecipe<M: Refs = Resolved> {
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub show_notification: bool,
    #[serde(default, skip_serializing_if = "is_default")]
    pub category: CraftingBookCategory,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub group: String,
    pub input: Ingredient<M>,
    pub material: Ingredient<M>,
    #[serde(
        default = "default_material_count",
        skip_serializing_if = "is_default_material_count"
    )]
    pub material_count: MinMaxBounds<i32>,
    pub result: TransmuteResult<M>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub add_material_count_to_result: bool,
}

const MATERIAL_COUNT: RangeInclusive<i32> = 1..=8;

fn default_material_count() -> MinMaxBounds<i32> {
    MinMaxBounds {
        min: Some(1),
        max: Some(1),
    }
}

fn is_default_material_count(count: &MinMaxBounds<i32>) -> bool {
    *count == default_material_count()
}

impl<M: Refs> Validate for TransmuteRecipe<M> {
    fn validate(&self) -> Result<(), String> {
        contained_in(&self.material_count, MATERIAL_COUNT)
    }
}

/// The item a transmutation makes, its count and the components it adds; a
/// bare item id reads as that item alone.
#[registry_value]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(remote = "Self", deny_unknown_fields)]
pub struct TransmuteResult<M: Refs = Resolved> {
    #[serde(rename = "id", default, skip_serializing_if = "Option::is_none")]
    pub item: Option<M::Id<Item>>,
    #[serde(default, skip_serializing_if = "is_default")]
    pub count: codec::Bounded<1, 99, 1>,
    #[serde(default, skip_serializing_if = "ComponentPatch::is_empty")]
    pub components: ComponentPatch<M>,
}

impl<M: Refs> Default for TransmuteResult<M> {
    fn default() -> Self {
        TransmuteResult {
            item: None,
            count: codec::Bounded::default(),
            components: ComponentPatch::EMPTY,
        }
    }
}

impl Serialize for TransmuteResult<Named> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        TransmuteResult::serialize(self, s)
    }
}

impl<'de> Deserialize<'de> for TransmuteResult<Named> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct ResultVisitor;

        impl<'de> Visitor<'de> for ResultVisitor {
            type Value = TransmuteResult<Named>;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("an item id or a transmute result")
            }

            fn visit_str<E: serde::de::Error>(self, id: &str) -> Result<TransmuteResult<Named>, E> {
                let item = Key::deserialize(value::StrDeserializer::<E>::new(id))?;
                Ok(TransmuteResult {
                    item: Some(item),
                    ..TransmuteResult::default()
                })
            }

            fn visit_map<A: MapAccess<'de>>(
                self,
                map: A,
            ) -> Result<TransmuteResult<Named>, A::Error> {
                TransmuteResult::deserialize(value::MapAccessDeserializer::new(map))
            }
        }

        d.deserialize_any(ResultVisitor)
    }
}

#[registry_value]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DecoratedPotRecipe<M: Refs = Resolved> {
    pub back: Ingredient<M>,
    pub left: Ingredient<M>,
    pub right: Ingredient<M>,
    pub front: Ingredient<M>,
    pub result: Template<M>,
}

validated!(BookCloningRecipe<Named>);

#[registry_value]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(remote = "Self", deny_unknown_fields)]
pub struct BookCloningRecipe<M: Refs = Resolved> {
    pub source: Ingredient<M>,
    pub material: Ingredient<M>,
    #[serde(
        default = "default_generations",
        skip_serializing_if = "is_default_generations"
    )]
    pub allowed_generations: MinMaxBounds<i32>,
    pub result: Template<M>,
}

const GENERATIONS: RangeInclusive<i32> = 0..=2;

fn default_generations() -> MinMaxBounds<i32> {
    MinMaxBounds {
        min: Some(0),
        max: Some(1),
    }
}

fn is_default_generations(generations: &MinMaxBounds<i32>) -> bool {
    *generations == default_generations()
}

impl<M: Refs> Validate for BookCloningRecipe<M> {
    fn validate(&self) -> Result<(), String> {
        contained_in(&self.allowed_generations, GENERATIONS)
    }
}

#[registry_value]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MapExtendingRecipe<M: Refs = Resolved> {
    pub map: Ingredient<M>,
    pub material: Ingredient<M>,
    pub result: TransmuteResult<M>,
}

#[registry_value]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FireworkRocketRecipe<M: Refs = Resolved> {
    pub shell: Ingredient<M>,
    pub fuel: Ingredient<M>,
    pub star: Ingredient<M>,
    pub result: Template<M>,
}

#[registry_value]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FireworkStarRecipe<M: Refs = Resolved> {
    pub shapes: FireworkShapes<M>,
    pub trail: Ingredient<M>,
    pub twinkle: Ingredient<M>,
    pub fuel: Ingredient<M>,
    pub dye: Ingredient<M>,
    pub result: Template<M>,
}

/// The ingredient that gives a star each explosion shape it can take.
#[registry_value]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FireworkShapes<M: Refs = Resolved> {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub small_ball: Option<Ingredient<M>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub large_ball: Option<Ingredient<M>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub star: Option<Ingredient<M>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub creeper: Option<Ingredient<M>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub burst: Option<Ingredient<M>>,
}

impl<M: Refs> Default for FireworkShapes<M> {
    fn default() -> Self {
        FireworkShapes {
            small_ball: None,
            large_ball: None,
            star: None,
            creeper: None,
            burst: None,
        }
    }
}

#[registry_value]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FireworkStarFadeRecipe<M: Refs = Resolved> {
    pub target: Ingredient<M>,
    pub dye: Ingredient<M>,
    pub result: Template<M>,
}

#[registry_value]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BannerDuplicateRecipe<M: Refs = Resolved> {
    pub banner: Ingredient<M>,
    pub result: Template<M>,
}

#[registry_value]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShieldDecorationRecipe<M: Refs = Resolved> {
    pub banner: Ingredient<M>,
    pub target: Ingredient<M>,
    pub result: Template<M>,
}

#[registry_value]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CookingRecipe<M: Refs = Resolved> {
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub show_notification: bool,
    #[serde(default, skip_serializing_if = "is_default")]
    pub category: CookingBookCategory,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub group: String,
    pub ingredient: Ingredient<M>,
    pub result: Template<M>,
    #[serde(default, skip_serializing_if = "is_default")]
    pub experience: f32,
    #[serde(rename = "cookingtime")]
    pub cooking_time: i32,
}

#[registry_value]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StonecutterRecipe<M: Refs = Resolved> {
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub show_notification: bool,
    pub ingredient: Ingredient<M>,
    pub result: Template<M>,
}

#[registry_value]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SmithingTransformRecipe<M: Refs = Resolved> {
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub show_notification: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub template: Option<Ingredient<M>>,
    pub base: Ingredient<M>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub addition: Option<Ingredient<M>>,
    pub result: Template<M>,
}

#[registry_value]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SmithingTrimRecipe<M: Refs = Resolved> {
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub show_notification: bool,
    pub template: Ingredient<M>,
    pub base: Ingredient<M>,
    pub addition: Ingredient<M>,
    pub pattern: TrimPatternRef<M>,
}

#[registry_value]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BrewingRecipe<M: Refs = Resolved> {
    pub input: PotionIngredient<M>,
    pub reagent: PotionIngredient<M>,
    pub output: Template<M>,
}

/// An ingredient that must also hold one of the potions or effects stated.
#[registry_value]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PotionIngredient<M: Refs = Resolved> {
    pub item: Ingredient<M>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub potion_contents: Option<PotionsPredicate<M>>,
}

fn contained_in(bounds: &MinMaxBounds<i32>, allowed: RangeInclusive<i32>) -> Result<(), String> {
    match (bounds.min, bounds.max) {
        (Some(low), Some(high)) if allowed.contains(&low) && allowed.contains(&high) => Ok(()),
        _ => Err(format!(
            "Range must be within [{}..{}], but was {}",
            allowed.start(),
            allowed.end(),
            range_text(bounds)
        )),
    }
}

fn range_text(bounds: &MinMaxBounds<i32>) -> String {
    match (bounds.min, bounds.max) {
        (Some(min), Some(max)) => format!("[{min}..{max}]"),
        (Some(min), None) => format!("[{min}..+\u{221e})"),
        (None, Some(max)) => format!("(-\u{221e}..{max}]"),
        (None, None) => "(-\u{221e}..+\u{221e})".to_owned(),
    }
}
