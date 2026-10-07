mod ingredient;
mod pattern;

use std::fmt;
use std::ops::RangeInclusive;

use crate::component::common::MinMaxBounds;
use crate::component::predicate::PotionsPredicate;
use crate::keys::Item;
use crate::patch::ComponentPatch;
use crate::{Template, TrimPattern};
use mcrs_minecraft_core::ResourceKey;
use mcrs_minecraft_core::codec::{self, Validate, default_true, is_default, is_true};
use mcrs_minecraft_core::registry_key::RegistryValue;
use mcrs_minecraft_core::validated;
use mcrs_minecraft_registry::Holder;
use serde::de::{MapAccess, Visitor, value};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

pub use ingredient::Ingredient;
pub use pattern::{Pattern, PatternKey, check_pattern};

/// A recipe as its data pack file states it, dispatched on its serializer.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(remote = "Self")]
pub enum Recipe {
    CraftingShaped(ShapedRecipe),
    CraftingShapeless(ShapelessRecipe),
    CraftingDye(DyeRecipe),
    CraftingImbue(ImbueRecipe),
    CraftingTransmute(TransmuteRecipe),
    CraftingDecoratedPot(DecoratedPotRecipe),
    BookCloning(BookCloningRecipe),
    MapExtending(MapExtendingRecipe),
    FireworkRocket(FireworkRocketRecipe),
    FireworkStar(FireworkStarRecipe),
    FireworkStarFade(FireworkStarFadeRecipe),
    BannerDuplicate(BannerDuplicateRecipe),
    ShieldDecoration(ShieldDecorationRecipe),
    RepairItem,
    Smelting(CookingRecipe),
    Blasting(CookingRecipe),
    Smoking(CookingRecipe),
    CampfireCooking(CookingRecipe),
    Stonecutting(StonecutterRecipe),
    SmithingTransform(SmithingTransformRecipe),
    SmithingTrim(SmithingTrimRecipe),
    Brewing(BrewingRecipe),
}

mcrs_minecraft_registry::dispatch! {
    Recipe, key = "type", registry = crate::keys::RecipeSerializer,
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

validated!(ShapedRecipe);

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(remote = "Self", deny_unknown_fields)]
pub struct ShapedRecipe {
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub show_notification: bool,
    #[serde(default, skip_serializing_if = "is_default")]
    pub category: CraftingBookCategory,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub group: String,
    pub key: PatternKey,
    pub pattern: Pattern,
    pub result: Template,
}

impl Validate for ShapedRecipe {
    fn validate(&self) -> Result<(), String> {
        check_pattern(&self.key, &self.pattern)
    }
}

validated!(ShapelessRecipe);

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(remote = "Self", deny_unknown_fields)]
pub struct ShapelessRecipe {
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub show_notification: bool,
    #[serde(default, skip_serializing_if = "is_default")]
    pub category: CraftingBookCategory,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub group: String,
    pub result: Template,
    pub ingredients: Vec<Ingredient>,
}

impl Validate for ShapelessRecipe {
    fn validate(&self) -> Result<(), String> {
        let count = self.ingredients.len();
        if (1..=9).contains(&count) {
            Ok(())
        } else {
            Err(format!(
                "List must have between 1 and 9 elements, but has {count}"
            ))
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DyeRecipe {
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub show_notification: bool,
    #[serde(default, skip_serializing_if = "is_default")]
    pub category: CraftingBookCategory,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub group: String,
    pub target: Ingredient,
    pub dye: Ingredient,
    pub result: Template,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImbueRecipe {
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub show_notification: bool,
    #[serde(default, skip_serializing_if = "is_default")]
    pub category: CraftingBookCategory,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub group: String,
    pub source: Ingredient,
    pub material: Ingredient,
    pub result: Template,
}

validated!(TransmuteRecipe);

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(remote = "Self", deny_unknown_fields)]
pub struct TransmuteRecipe {
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub show_notification: bool,
    #[serde(default, skip_serializing_if = "is_default")]
    pub category: CraftingBookCategory,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub group: String,
    pub input: Ingredient,
    pub material: Ingredient,
    #[serde(
        default = "TransmuteRecipe::default_material_count",
        skip_serializing_if = "TransmuteRecipe::is_default_material_count"
    )]
    pub material_count: MinMaxBounds<i32>,
    pub result: TransmuteResult,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub add_material_count_to_result: bool,
}

impl TransmuteRecipe {
    const MATERIAL_COUNT: RangeInclusive<i32> = 1..=8;

    fn default_material_count() -> MinMaxBounds<i32> {
        MinMaxBounds {
            min: Some(1),
            max: Some(1),
        }
    }

    fn is_default_material_count(count: &MinMaxBounds<i32>) -> bool {
        *count == Self::default_material_count()
    }
}

impl Validate for TransmuteRecipe {
    fn validate(&self) -> Result<(), String> {
        contained_in(&self.material_count, Self::MATERIAL_COUNT)
    }
}

/// The item a transmutation makes, its count and the components it adds; a
/// bare item id reads as that item alone.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(remote = "Self", deny_unknown_fields)]
pub struct TransmuteResult {
    #[serde(rename = "id", default, skip_serializing_if = "Option::is_none")]
    pub item: Option<ResourceKey<Item>>,
    #[serde(default, skip_serializing_if = "is_default")]
    pub count: codec::Bounded<1, 99, 1>,
    #[serde(default, skip_serializing_if = "ComponentPatch::is_empty")]
    pub components: ComponentPatch,
}

impl Serialize for TransmuteResult {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        TransmuteResult::serialize(self, s)
    }
}

impl<'de> Deserialize<'de> for TransmuteResult {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct ResultVisitor;

        impl<'de> Visitor<'de> for ResultVisitor {
            type Value = TransmuteResult;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("an item id or a transmute result")
            }

            fn visit_str<E: serde::de::Error>(self, id: &str) -> Result<TransmuteResult, E> {
                let item = ResourceKey::deserialize(value::StrDeserializer::<E>::new(id))?;
                Ok(TransmuteResult {
                    item: Some(item),
                    ..TransmuteResult::default()
                })
            }

            fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<TransmuteResult, A::Error> {
                TransmuteResult::deserialize(value::MapAccessDeserializer::new(map))
            }
        }

        d.deserialize_any(ResultVisitor)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DecoratedPotRecipe {
    pub back: Ingredient,
    pub left: Ingredient,
    pub right: Ingredient,
    pub front: Ingredient,
    pub result: Template,
}

validated!(BookCloningRecipe);

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(remote = "Self", deny_unknown_fields)]
pub struct BookCloningRecipe {
    pub source: Ingredient,
    pub material: Ingredient,
    #[serde(
        default = "BookCloningRecipe::default_generations",
        skip_serializing_if = "BookCloningRecipe::is_default_generations"
    )]
    pub allowed_generations: MinMaxBounds<i32>,
    pub result: Template,
}

impl BookCloningRecipe {
    const GENERATIONS: RangeInclusive<i32> = 0..=2;

    fn default_generations() -> MinMaxBounds<i32> {
        MinMaxBounds {
            min: Some(0),
            max: Some(1),
        }
    }

    fn is_default_generations(generations: &MinMaxBounds<i32>) -> bool {
        *generations == Self::default_generations()
    }
}

impl Validate for BookCloningRecipe {
    fn validate(&self) -> Result<(), String> {
        contained_in(&self.allowed_generations, Self::GENERATIONS)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MapExtendingRecipe {
    pub map: Ingredient,
    pub material: Ingredient,
    pub result: TransmuteResult,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FireworkRocketRecipe {
    pub shell: Ingredient,
    pub fuel: Ingredient,
    pub star: Ingredient,
    pub result: Template,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FireworkStarRecipe {
    pub shapes: FireworkShapes,
    pub trail: Ingredient,
    pub twinkle: Ingredient,
    pub fuel: Ingredient,
    pub dye: Ingredient,
    pub result: Template,
}

/// The ingredient that gives a star each explosion shape it can take.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FireworkShapes {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub small_ball: Option<Ingredient>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub large_ball: Option<Ingredient>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub star: Option<Ingredient>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub creeper: Option<Ingredient>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub burst: Option<Ingredient>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FireworkStarFadeRecipe {
    pub target: Ingredient,
    pub dye: Ingredient,
    pub result: Template,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BannerDuplicateRecipe {
    pub banner: Ingredient,
    pub result: Template,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShieldDecorationRecipe {
    pub banner: Ingredient,
    pub target: Ingredient,
    pub result: Template,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CookingRecipe {
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub show_notification: bool,
    #[serde(default, skip_serializing_if = "is_default")]
    pub category: CookingBookCategory,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub group: String,
    pub ingredient: Ingredient,
    pub result: Template,
    #[serde(default, skip_serializing_if = "is_default")]
    pub experience: f32,
    #[serde(rename = "cookingtime")]
    pub cooking_time: i32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StonecutterRecipe {
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub show_notification: bool,
    pub ingredient: Ingredient,
    pub result: Template,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SmithingTransformRecipe {
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub show_notification: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub template: Option<Ingredient>,
    pub base: Ingredient,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub addition: Option<Ingredient>,
    pub result: Template,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SmithingTrimRecipe {
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub show_notification: bool,
    pub template: Ingredient,
    pub base: Ingredient,
    pub addition: Ingredient,
    pub pattern: Holder<TrimPattern>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BrewingRecipe {
    pub input: PotionIngredient,
    pub reagent: PotionIngredient,
    pub output: Template,
}

/// An ingredient that must also hold one of the potions or effects stated.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PotionIngredient {
    pub item: Ingredient,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub potion_contents: Option<PotionsPredicate>,
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
