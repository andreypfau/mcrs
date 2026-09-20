use std::io::Write;

use mcrs_minecraft_core::codec::Validate;
use mcrs_minecraft_core::{HolderSet, ResourceKey, validated};
use mcrs_minecraft_protocol_macros::{Decode, Encode};
use mcrs_minecraft_registry::RegistryLookup;
use serde::{Deserialize, Serialize};

use crate::entity::OptionalUnsignedInt;
use crate::item::ctx::nested;
use crate::item::{
    DecodeCtx, EncodeCtx, Holder, ItemComponentKind, ItemReg, Template, TrimPattern,
};
use crate::{Decode as _, Encode as _, VarInt};

validated!(Ingredient);

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(remote = "Self", transparent)]
pub struct Ingredient(pub HolderSet<ResourceKey<ItemReg>>);

impl Ingredient {
    pub fn new(values: HolderSet<ResourceKey<ItemReg>>) -> Result<Self, String> {
        let ingredient = Ingredient(values);
        ingredient.validate()?;
        Ok(ingredient)
    }
}

impl Validate for Ingredient {
    fn validate(&self) -> Result<(), String> {
        if matches!(self.0, HolderSet::Tag(_)) {
            return Ok(());
        }
        let entries = self.0.entries();
        if entries.is_empty() {
            return Err("Ingredients can't be empty".into());
        }
        if entries.iter().any(|item| item.as_str() == "minecraft:air") {
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
        Ingredient::new(HolderSet::decode_ctx(ctx, r)?).map_err(anyhow::Error::msg)
    }
}

/// `minecraft:slot_display`, whose ids are the wire dispatch prefix.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub enum SlotDisplayType {
    Empty,
    AnyFuel,
    WithAnyPotion,
    OnlyWithComponent,
    Item,
    ItemStack,
    Tag,
    Dyed,
    SmithingTrim,
    WithRemainder,
    Composite,
}

impl SlotDisplayType {
    pub const ALL: [Self; 11] = [
        Self::Empty,
        Self::AnyFuel,
        Self::WithAnyPotion,
        Self::OnlyWithComponent,
        Self::Item,
        Self::ItemStack,
        Self::Tag,
        Self::Dyed,
        Self::SmithingTrim,
        Self::WithRemainder,
        Self::Composite,
    ];

    pub const fn id(self) -> &'static str {
        match self {
            Self::Empty => "minecraft:empty",
            Self::AnyFuel => "minecraft:any_fuel",
            Self::WithAnyPotion => "minecraft:with_any_potion",
            Self::OnlyWithComponent => "minecraft:only_with_component",
            Self::Item => "minecraft:item",
            Self::ItemStack => "minecraft:item_stack",
            Self::Tag => "minecraft:tag",
            Self::Dyed => "minecraft:dyed",
            Self::SmithingTrim => "minecraft:smithing_trim",
            Self::WithRemainder => "minecraft:with_remainder",
            Self::Composite => "minecraft:composite",
        }
    }
}

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
    Item { item: ResourceKey<ItemReg> },
    #[serde(rename = "minecraft:item_stack", alias = "item_stack")]
    ItemStack { item: Template },
    #[serde(rename = "minecraft:tag", alias = "tag")]
    Tag {
        tag: HolderSet<ResourceKey<ItemReg>>,
    },
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

/// `minecraft:recipe_display`, whose ids are the wire dispatch prefix.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub enum RecipeDisplayType {
    CraftingShapeless,
    CraftingShaped,
    Furnace,
    Stonecutter,
    Smithing,
}

impl RecipeDisplayType {
    pub const ALL: [Self; 5] = [
        Self::CraftingShapeless,
        Self::CraftingShaped,
        Self::Furnace,
        Self::Stonecutter,
        Self::Smithing,
    ];

    pub const fn id(self) -> &'static str {
        match self {
            Self::CraftingShapeless => "minecraft:crafting_shapeless",
            Self::CraftingShaped => "minecraft:crafting_shaped",
            Self::Furnace => "minecraft:furnace",
            Self::Stonecutter => "minecraft:stonecutter",
            Self::Smithing => "minecraft:smithing",
        }
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

impl RecipeBookCategory {
    pub const ALL: [Self; 13] = [
        Self::CraftingBuildingBlocks,
        Self::CraftingRedstone,
        Self::CraftingEquipment,
        Self::CraftingMisc,
        Self::FurnaceFood,
        Self::FurnaceBlocks,
        Self::FurnaceMisc,
        Self::BlastFurnaceBlocks,
        Self::BlastFurnaceMisc,
        Self::SmokerFood,
        Self::Stonecutter,
        Self::Smithing,
        Self::Campfire,
    ];

    pub const fn id(self) -> &'static str {
        match self {
            Self::CraftingBuildingBlocks => "minecraft:crafting_building_blocks",
            Self::CraftingRedstone => "minecraft:crafting_redstone",
            Self::CraftingEquipment => "minecraft:crafting_equipment",
            Self::CraftingMisc => "minecraft:crafting_misc",
            Self::FurnaceFood => "minecraft:furnace_food",
            Self::FurnaceBlocks => "minecraft:furnace_blocks",
            Self::FurnaceMisc => "minecraft:furnace_misc",
            Self::BlastFurnaceBlocks => "minecraft:blast_furnace_blocks",
            Self::BlastFurnaceMisc => "minecraft:blast_furnace_misc",
            Self::SmokerFood => "minecraft:smoker_food",
            Self::Stonecutter => "minecraft:stonecutter",
            Self::Smithing => "minecraft:smithing",
            Self::Campfire => "minecraft:campfire",
        }
    }
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
pub type RecipePropertySet = Vec<ResourceKey<ItemReg>>;

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
