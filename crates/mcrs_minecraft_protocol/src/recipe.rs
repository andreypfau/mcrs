use crate::keys::{RecipeDisplayType, SlotDisplayType};
use std::io::Write;

use anyhow::ensure;
use mcrs_minecraft_core::codec::Validate;
use mcrs_minecraft_core::{ResourceKey, validated};
use mcrs_minecraft_keys::Item;
use mcrs_minecraft_protocol_macros::{Decode, Encode};
use mcrs_minecraft_registry::{HolderSet, RegistryLookup, skipping_sets};
use serde::{Deserialize, Serialize};

use crate::entity::OptionalUnsignedInt;
use crate::item::ctx::nested;
use crate::item::{DecodeCtx, EncodeCtx, Holder, ItemComponentKind, Template, TrimPattern};
use crate::{Decode as _, Encode as _, VarInt};

validated!(Ingredient);

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(remote = "Self", transparent)]
pub struct Ingredient(pub HolderSet<Item>);

const AIR_ITEM_ID: u16 = 0;

impl Validate for Ingredient {
    fn validate(&self) -> Result<(), String> {
        let entries = match &self.0 {
            HolderSet::Named(_) => return Ok(()),
            HolderSet::One(item) => std::slice::from_ref(item),
            HolderSet::List(items) => &items[..],
        };
        if entries.is_empty() {
            return Err("Ingredients can't be empty".into());
        }
        if entries.iter().any(|item| item.number() == AIR_ITEM_ID) {
            return Err("Ingredient can't contain air".into());
        }
        Ok(())
    }
}

impl EncodeCtx for Ingredient {
    fn encode_ctx(&self, ctx: &dyn RegistryLookup, w: impl Write) -> anyhow::Result<()> {
        self.validate().map_err(anyhow::Error::msg)?;
        self.0.encode_ctx(ctx, w)
    }
}

impl DecodeCtx<'_> for Ingredient {
    fn decode_ctx(ctx: &dyn RegistryLookup, r: &mut &[u8]) -> anyhow::Result<Self> {
        let announced = VarInt::decode(&mut &r[..])?.0;
        let ingredient = Ingredient(HolderSet::decode_ctx(ctx, r)?);
        if skipping_sets() {
            ensure!(announced != 1, "Ingredients can't be empty");
            return Ok(ingredient);
        }
        ingredient.validate().map_err(anyhow::Error::msg)?;
        Ok(ingredient)
    }
}

macro_rules! static_registry_wire {
    ($($ty:ty),* $(,)?) => {$(
        impl crate::Encode for $ty {
            fn encode(&self, w: impl std::io::Write) -> anyhow::Result<()> {
                crate::VarInt(i32::from(*self as u16)).encode(w)
            }
        }

        impl<'a> crate::Decode<'a> for $ty {
            fn decode(r: &mut &'a [u8]) -> anyhow::Result<Self> {
                let id = crate::VarInt::decode(r)?.0;
                u16::try_from(id)
                    .ok()
                    .and_then(<$ty>::from_protocol_id)
                    .ok_or_else(|| {
                        anyhow::anyhow!("unexpected enum discriminant {id} in `{}`", stringify!($ty))
                    })
            }
        }
    )*};
}

static_registry_wire!(SlotDisplayType, RecipeDisplayType);

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", deny_unknown_fields)]
pub enum SlotDisplay {
    #[serde(rename = "minecraft:empty", alias = "empty")]
    Empty,
    #[serde(rename = "minecraft:any_fuel", alias = "any_fuel")]
    AnyFuel,
    #[serde(rename = "minecraft:with_any_potion", alias = "with_any_potion")]
    WithAnyPotion { contents: Box<SlotDisplay> },
    #[serde(
        rename = "minecraft:only_with_component",
        alias = "only_with_component"
    )]
    OnlyWithComponent {
        contents: Box<SlotDisplay>,
        component: ItemComponentKind,
    },
    #[serde(rename = "minecraft:item", alias = "item")]
    Item { item: ResourceKey<Item> },
    #[serde(rename = "minecraft:item_stack", alias = "item_stack")]
    ItemStack { item: Template },
    #[serde(rename = "minecraft:tag", alias = "tag")]
    Tag { tag: HolderSet<Item> },
    #[serde(rename = "minecraft:dyed", alias = "dyed")]
    Dyed {
        dye: Box<SlotDisplay>,
        target: Box<SlotDisplay>,
    },
    #[serde(rename = "minecraft:smithing_trim", alias = "smithing_trim")]
    SmithingTrim {
        base: Box<SlotDisplay>,
        material: Box<SlotDisplay>,
        pattern: Holder<TrimPattern>,
    },
    #[serde(rename = "minecraft:with_remainder", alias = "with_remainder")]
    WithRemainder {
        input: Box<SlotDisplay>,
        remainder: Box<SlotDisplay>,
    },
    #[serde(rename = "minecraft:composite", alias = "composite")]
    Composite { contents: Vec<SlotDisplay> },
}

const SLOT_DISPLAY_ROWS: &[&str] = &[
    "minecraft:empty",
    "minecraft:any_fuel",
    "minecraft:with_any_potion",
    "minecraft:only_with_component",
    "minecraft:item",
    "minecraft:item_stack",
    "minecraft:tag",
    "minecraft:dyed",
    "minecraft:smithing_trim",
    "minecraft:with_remainder",
    "minecraft:composite",
];

const _: () = assert!(mcrs_minecraft_registry::static_rows::names_cover(
    SLOT_DISPLAY_ROWS,
    &[],
    crate::keys::SlotDisplayType::ENTRIES
));

impl SlotDisplay {
    pub fn kind(&self) -> SlotDisplayType {
        match self {
            Self::Empty => SlotDisplayType::Empty,
            Self::AnyFuel => SlotDisplayType::AnyFuel,
            Self::WithAnyPotion { .. } => SlotDisplayType::WithAnyPotion,
            Self::OnlyWithComponent { .. } => SlotDisplayType::OnlyWithComponent,
            Self::Item { .. } => SlotDisplayType::Item,
            Self::ItemStack { .. } => SlotDisplayType::ItemStack,
            Self::Tag { .. } => SlotDisplayType::Tag,
            Self::Dyed { .. } => SlotDisplayType::Dyed,
            Self::SmithingTrim { .. } => SlotDisplayType::SmithingTrim,
            Self::WithRemainder { .. } => SlotDisplayType::WithRemainder,
            Self::Composite { .. } => SlotDisplayType::Composite,
        }
    }
}

impl EncodeCtx for SlotDisplay {
    fn encode_ctx(&self, ctx: &dyn RegistryLookup, mut w: impl Write) -> anyhow::Result<()> {
        let w: &mut dyn Write = &mut w;
        self.kind().encode(&mut *w)?;
        match self {
            Self::Empty | Self::AnyFuel => Ok(()),
            Self::WithAnyPotion { contents } => contents.encode_ctx(ctx, w),
            Self::OnlyWithComponent {
                contents,
                component,
            } => {
                contents.encode_ctx(ctx, &mut *w)?;
                component.encode(w)
            }
            Self::Item { item } => item.encode_ctx(ctx, w),
            Self::ItemStack { item } => item.encode_ctx(ctx, w),
            Self::Tag { tag } => tag.encode_ctx(ctx, w),
            Self::Dyed { dye, target } => {
                dye.encode_ctx(ctx, &mut *w)?;
                target.encode_ctx(ctx, w)
            }
            Self::SmithingTrim {
                base,
                material,
                pattern,
            } => {
                base.encode_ctx(ctx, &mut *w)?;
                material.encode_ctx(ctx, &mut *w)?;
                pattern.encode_ctx(ctx, w)
            }
            Self::WithRemainder { input, remainder } => {
                input.encode_ctx(ctx, &mut *w)?;
                remainder.encode_ctx(ctx, w)
            }
            Self::Composite { contents } => contents.encode_ctx(ctx, w),
        }
    }
}

impl DecodeCtx<'_> for SlotDisplay {
    fn decode_ctx(ctx: &dyn RegistryLookup, r: &mut &[u8]) -> anyhow::Result<Self> {
        nested(|| {
            let boxed = |r: &mut &[u8]| SlotDisplay::decode_ctx(ctx, r).map(Box::new);
            Ok(match SlotDisplayType::decode(r)? {
                SlotDisplayType::Empty => Self::Empty,
                SlotDisplayType::AnyFuel => Self::AnyFuel,
                SlotDisplayType::WithAnyPotion => Self::WithAnyPotion {
                    contents: boxed(r)?,
                },
                SlotDisplayType::OnlyWithComponent => Self::OnlyWithComponent {
                    contents: boxed(r)?,
                    component: ItemComponentKind::decode(r)?,
                },
                SlotDisplayType::Item => Self::Item {
                    item: ResourceKey::decode_ctx(ctx, r)?,
                },
                SlotDisplayType::ItemStack => Self::ItemStack {
                    item: Template::decode_ctx(ctx, r)?,
                },
                SlotDisplayType::Tag => Self::Tag {
                    tag: HolderSet::decode_ctx(ctx, r)?,
                },
                SlotDisplayType::Dyed => Self::Dyed {
                    dye: boxed(r)?,
                    target: boxed(r)?,
                },
                SlotDisplayType::SmithingTrim => Self::SmithingTrim {
                    base: boxed(r)?,
                    material: boxed(r)?,
                    pattern: Holder::decode_ctx(ctx, r)?,
                },
                SlotDisplayType::WithRemainder => Self::WithRemainder {
                    input: boxed(r)?,
                    remainder: boxed(r)?,
                },
                SlotDisplayType::Composite => Self::Composite {
                    contents: Vec::decode_ctx(ctx, r)?,
                },
            })
        })
    }
}

validated!(RecipeDisplay);

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(remote = "Self", tag = "type", deny_unknown_fields)]
pub enum RecipeDisplay {
    #[serde(rename = "minecraft:crafting_shapeless", alias = "crafting_shapeless")]
    CraftingShapeless {
        ingredients: Vec<SlotDisplay>,
        result: SlotDisplay,
        crafting_station: SlotDisplay,
    },
    #[serde(rename = "minecraft:crafting_shaped", alias = "crafting_shaped")]
    CraftingShaped {
        width: i32,
        height: i32,
        ingredients: Vec<SlotDisplay>,
        result: SlotDisplay,
        crafting_station: SlotDisplay,
    },
    #[serde(rename = "minecraft:furnace", alias = "furnace")]
    Furnace {
        ingredient: SlotDisplay,
        fuel: SlotDisplay,
        result: SlotDisplay,
        crafting_station: SlotDisplay,
        duration: i32,
        experience: f32,
    },
    #[serde(rename = "minecraft:stonecutter", alias = "stonecutter")]
    Stonecutter {
        input: SlotDisplay,
        result: SlotDisplay,
        crafting_station: SlotDisplay,
    },
    #[serde(rename = "minecraft:smithing", alias = "smithing")]
    Smithing {
        template: SlotDisplay,
        base: SlotDisplay,
        addition: SlotDisplay,
        result: SlotDisplay,
        crafting_station: SlotDisplay,
    },
}

const RECIPE_DISPLAY_ROWS: &[&str] = &[
    "minecraft:crafting_shapeless",
    "minecraft:crafting_shaped",
    "minecraft:furnace",
    "minecraft:stonecutter",
    "minecraft:smithing",
];

const _: () = assert!(mcrs_minecraft_registry::static_rows::names_cover(
    RECIPE_DISPLAY_ROWS,
    &[],
    crate::keys::RecipeDisplayType::ENTRIES
));

impl Validate for RecipeDisplay {
    fn validate(&self) -> Result<(), String> {
        if let Self::CraftingShaped {
            width,
            height,
            ingredients,
            ..
        } = self
            && ingredients.len() as i64 != *width as i64 * *height as i64
        {
            return Err("Invalid shaped recipe display contents".into());
        }
        Ok(())
    }
}

impl RecipeDisplay {
    pub fn kind(&self) -> RecipeDisplayType {
        match self {
            Self::CraftingShapeless { .. } => RecipeDisplayType::CraftingShapeless,
            Self::CraftingShaped { .. } => RecipeDisplayType::CraftingShaped,
            Self::Furnace { .. } => RecipeDisplayType::Furnace,
            Self::Stonecutter { .. } => RecipeDisplayType::Stonecutter,
            Self::Smithing { .. } => RecipeDisplayType::Smithing,
        }
    }
}

impl EncodeCtx for RecipeDisplay {
    fn encode_ctx(&self, ctx: &dyn RegistryLookup, mut w: impl Write) -> anyhow::Result<()> {
        self.validate().map_err(anyhow::Error::msg)?;
        self.kind().encode(&mut w)?;
        match self {
            Self::CraftingShapeless {
                ingredients,
                result,
                crafting_station,
            } => {
                ingredients.encode_ctx(ctx, &mut w)?;
                result.encode_ctx(ctx, &mut w)?;
                crafting_station.encode_ctx(ctx, w)
            }
            Self::CraftingShaped {
                width,
                height,
                ingredients,
                result,
                crafting_station,
            } => {
                VarInt(*width).encode(&mut w)?;
                VarInt(*height).encode(&mut w)?;
                ingredients.encode_ctx(ctx, &mut w)?;
                result.encode_ctx(ctx, &mut w)?;
                crafting_station.encode_ctx(ctx, w)
            }
            Self::Furnace {
                ingredient,
                fuel,
                result,
                crafting_station,
                duration,
                experience,
            } => {
                ingredient.encode_ctx(ctx, &mut w)?;
                fuel.encode_ctx(ctx, &mut w)?;
                result.encode_ctx(ctx, &mut w)?;
                crafting_station.encode_ctx(ctx, &mut w)?;
                VarInt(*duration).encode(&mut w)?;
                experience.encode(w)
            }
            Self::Stonecutter {
                input,
                result,
                crafting_station,
            } => {
                input.encode_ctx(ctx, &mut w)?;
                result.encode_ctx(ctx, &mut w)?;
                crafting_station.encode_ctx(ctx, w)
            }
            Self::Smithing {
                template,
                base,
                addition,
                result,
                crafting_station,
            } => {
                template.encode_ctx(ctx, &mut w)?;
                base.encode_ctx(ctx, &mut w)?;
                addition.encode_ctx(ctx, &mut w)?;
                result.encode_ctx(ctx, &mut w)?;
                crafting_station.encode_ctx(ctx, w)
            }
        }
    }
}

impl DecodeCtx<'_> for RecipeDisplay {
    fn decode_ctx(ctx: &dyn RegistryLookup, r: &mut &[u8]) -> anyhow::Result<Self> {
        let slot = |r: &mut &[u8]| SlotDisplay::decode_ctx(ctx, r);
        let display = match RecipeDisplayType::decode(r)? {
            RecipeDisplayType::CraftingShapeless => Self::CraftingShapeless {
                ingredients: Vec::decode_ctx(ctx, r)?,
                result: slot(r)?,
                crafting_station: slot(r)?,
            },
            RecipeDisplayType::CraftingShaped => Self::CraftingShaped {
                width: VarInt::decode(r)?.0,
                height: VarInt::decode(r)?.0,
                ingredients: Vec::decode_ctx(ctx, r)?,
                result: slot(r)?,
                crafting_station: slot(r)?,
            },
            RecipeDisplayType::Furnace => Self::Furnace {
                ingredient: slot(r)?,
                fuel: slot(r)?,
                result: slot(r)?,
                crafting_station: slot(r)?,
                duration: VarInt::decode(r)?.0,
                experience: f32::decode(r)?,
            },
            RecipeDisplayType::Stonecutter => Self::Stonecutter {
                input: slot(r)?,
                result: slot(r)?,
                crafting_station: slot(r)?,
            },
            RecipeDisplayType::Smithing => Self::Smithing {
                template: slot(r)?,
                base: slot(r)?,
                addition: slot(r)?,
                result: slot(r)?,
                crafting_station: slot(r)?,
            },
        };
        display.validate().map_err(anyhow::Error::msg)?;
        Ok(display)
    }
}

/// `minecraft:recipe_book_category`, whose ids are the wire form.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub enum RecipeBookCategory {
    CraftingBuildingBlocks,
    CraftingRedstone,
    CraftingEquipment,
    CraftingMisc,
    FurnaceFood,
    FurnaceBlocks,
    FurnaceMisc,
    BlastFurnaceBlocks,
    BlastFurnaceMisc,
    SmokerFood,
    Stonecutter,
    Smithing,
    Campfire,
}

/// One recipe as the client's recipe book shows it; `group` is the index of
/// its group, `crafting_requirements` is absent for recipes the book cannot
/// place, and `flags` carries [`Self::FLAG_NOTIFICATION`] and
/// [`Self::FLAG_HIGHLIGHT`].
#[derive(Clone, Debug, PartialEq)]
pub struct RecipeBookEntry {
    pub id: VarInt,
    pub display: RecipeDisplay,
    pub group: Option<u32>,
    pub category: RecipeBookCategory,
    pub crafting_requirements: Option<Vec<Ingredient>>,
    pub flags: u8,
}

impl RecipeBookEntry {
    pub const FLAG_NOTIFICATION: u8 = 1;
    pub const FLAG_HIGHLIGHT: u8 = 2;
}

impl EncodeCtx for RecipeBookEntry {
    fn encode_ctx(&self, ctx: &dyn RegistryLookup, mut w: impl Write) -> anyhow::Result<()> {
        self.id.encode(&mut w)?;
        self.display.encode_ctx(ctx, &mut w)?;
        OptionalUnsignedInt(self.group).encode(&mut w)?;
        self.category.encode(&mut w)?;
        self.crafting_requirements.encode_ctx(ctx, &mut w)?;
        self.flags.encode(w)
    }
}

impl DecodeCtx<'_> for RecipeBookEntry {
    fn decode_ctx(ctx: &dyn RegistryLookup, r: &mut &[u8]) -> anyhow::Result<Self> {
        Ok(RecipeBookEntry {
            id: VarInt::decode(r)?,
            display: RecipeDisplay::decode_ctx(ctx, r)?,
            group: OptionalUnsignedInt::decode(r)?.0,
            category: RecipeBookCategory::decode(r)?,
            crafting_requirements: Option::decode_ctx(ctx, r)?,
            flags: u8::decode(r)?,
        })
    }
}

/// The items a `recipe_property_set` accepts, in the order vanilla's set
/// iterates them.
pub type RecipePropertySet = Vec<ResourceKey<Item>>;

/// One stonecutter option: the input it accepts and what the button shows.
#[derive(Clone, Debug, PartialEq)]
pub struct SelectableRecipe {
    pub input: Ingredient,
    pub option_display: SlotDisplay,
}

impl EncodeCtx for SelectableRecipe {
    fn encode_ctx(&self, ctx: &dyn RegistryLookup, mut w: impl Write) -> anyhow::Result<()> {
        self.input.encode_ctx(ctx, &mut w)?;
        self.option_display.encode_ctx(ctx, w)
    }
}

impl DecodeCtx<'_> for SelectableRecipe {
    fn decode_ctx(ctx: &dyn RegistryLookup, r: &mut &[u8]) -> anyhow::Result<Self> {
        Ok(SelectableRecipe {
            input: Ingredient::decode_ctx(ctx, r)?,
            option_display: SlotDisplay::decode_ctx(ctx, r)?,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub enum RecipeBookType {
    Crafting,
    Furnace,
    BlastFurnace,
    Smoker,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Encode, Decode)]
pub struct RecipeBookTypeSettings {
    pub open: bool,
    pub filtering: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Encode, Decode)]
pub struct RecipeBookSettings {
    pub crafting: RecipeBookTypeSettings,
    pub furnace: RecipeBookTypeSettings,
    pub blast_furnace: RecipeBookTypeSettings,
    pub smoker: RecipeBookTypeSettings,
}

#[cfg(test)]
mod dispatch_rows {
    use super::*;

    #[test]
    fn slot_display_rows_select_their_variants() {
        mcrs_minecraft_registry::static_rows::assert_dispatch::<SlotDisplay>(
            SLOT_DISPLAY_ROWS,
            &[],
            crate::keys::SlotDisplayType::ENTRIES,
            |name| serde_json::json!({ "type": name }),
        );
    }

    #[test]
    fn recipe_display_rows_select_their_variants() {
        mcrs_minecraft_registry::static_rows::assert_dispatch::<RecipeDisplay>(
            RECIPE_DISPLAY_ROWS,
            &[],
            crate::keys::RecipeDisplayType::ENTRIES,
            |name| serde_json::json!({ "type": name }),
        );
    }
}
