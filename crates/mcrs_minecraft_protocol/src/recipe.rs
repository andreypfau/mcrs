use crate::keys::{RecipeDisplayType, SlotDisplayType};
use mcrs_minecraft_item::keys::RecipeBookCategory;
use mcrs_minecraft_item::recipe::Ingredient;
use std::io::Write;

use anyhow::ensure;
use mcrs_minecraft_core::ResourceKey;
use mcrs_minecraft_core::codec::Validate;
use mcrs_minecraft_item::keys::Item;
use mcrs_minecraft_protocol_macros::{Decode, Encode};
use mcrs_minecraft_registry::{HolderSet, RegistryLookup, skipping_sets};
use serde::{Deserialize, Serialize};

use crate::entity::OptionalUnsignedInt;
use crate::item::ctx::nested;
use crate::item::{DecodeCtx, EncodeCtx, Holder, ItemComponentKind, Template, TrimPattern};
use crate::{Decode as _, Encode as _, VarInt};

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

static_registry_wire!(SlotDisplayType, RecipeDisplayType, RecipeBookCategory);

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(remote = "Self", deny_unknown_fields)]
pub enum SlotDisplay {
    Empty,
    AnyFuel,
    WithAnyPotion {
        contents: Box<SlotDisplay>,
    },
    OnlyWithComponent {
        contents: Box<SlotDisplay>,
        component: ItemComponentKind,
    },
    Item {
        item: ResourceKey<Item>,
    },
    ItemStack {
        item: Template,
    },
    Tag {
        tag: HolderSet<Item>,
    },
    Dyed {
        dye: Box<SlotDisplay>,
        target: Box<SlotDisplay>,
    },
    SmithingTrim {
        base: Box<SlotDisplay>,
        material: Box<SlotDisplay>,
        pattern: Holder<TrimPattern>,
    },
    WithRemainder {
        input: Box<SlotDisplay>,
        remainder: Box<SlotDisplay>,
    },
    Composite {
        contents: Vec<SlotDisplay>,
    },
}

mcrs_minecraft_registry::dispatch! {
    SlotDisplay, key = "type", registry = crate::keys::SlotDisplayType,
    {
        Empty => Empty,
        AnyFuel => AnyFuel,
        WithAnyPotion => WithAnyPotion,
        OnlyWithComponent => OnlyWithComponent,
        Item => Item,
        ItemStack => ItemStack,
        Tag => Tag,
        Dyed => Dyed,
        SmithingTrim => SmithingTrim,
        WithRemainder => WithRemainder,
        Composite => Composite,
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

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(remote = "Self", deny_unknown_fields)]
pub enum RecipeDisplay {
    CraftingShapeless {
        ingredients: Vec<SlotDisplay>,
        result: SlotDisplay,
        crafting_station: SlotDisplay,
    },
    CraftingShaped {
        width: i32,
        height: i32,
        ingredients: Vec<SlotDisplay>,
        result: SlotDisplay,
        crafting_station: SlotDisplay,
    },
    Furnace {
        ingredient: SlotDisplay,
        fuel: SlotDisplay,
        result: SlotDisplay,
        crafting_station: SlotDisplay,
        duration: i32,
        experience: f32,
    },
    Stonecutter {
        input: SlotDisplay,
        result: SlotDisplay,
        crafting_station: SlotDisplay,
    },
    Smithing {
        template: SlotDisplay,
        base: SlotDisplay,
        addition: SlotDisplay,
        result: SlotDisplay,
        crafting_station: SlotDisplay,
    },
}

mcrs_minecraft_registry::dispatch! {
    validated RecipeDisplay, key = "type", registry = crate::keys::RecipeDisplayType,
    {
        CraftingShapeless => CraftingShapeless,
        CraftingShaped => CraftingShaped,
        Furnace => Furnace,
        Stonecutter => Stonecutter,
        Smithing => Smithing,
    }
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
