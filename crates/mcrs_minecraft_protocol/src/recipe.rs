use crate::Result;
use mcrs_minecraft_item::keys::{RecipeBookCategory, RecipeDisplayType, SlotDisplayType};
use mcrs_minecraft_item::recipe::Ingredient;
use std::io::Write;

use mcrs_minecraft_core::codec::Validate;
use mcrs_minecraft_item::component::trim::TrimPatternRef;
use mcrs_minecraft_item::keys::Item;
use mcrs_minecraft_registries::{FromWire, Refs, Resolved, Wire};
use mcrs_minecraft_wire::{Decode, Encode};

use crate::VarInt;
use crate::entity::OptionalUnsignedInt;
use crate::item::Template;
use crate::wire::nested;
use mcrs_minecraft_item::keys::DataComponentType;

#[derive(Clone, Debug, PartialEq, FromWire)]
pub enum SlotDisplay<M: Refs = Resolved> {
    Empty,
    AnyFuel,
    WithAnyPotion {
        contents: Box<SlotDisplay<M>>,
    },
    OnlyWithComponent {
        contents: Box<SlotDisplay<M>>,
        component: DataComponentType,
    },
    Item {
        item: M::Id<Item>,
    },
    ItemStack {
        item: Template<M>,
    },
    Tag {
        tag: M::Set<Item>,
    },
    Dyed {
        dye: Box<SlotDisplay<M>>,
        target: Box<SlotDisplay<M>>,
    },
    SmithingTrim {
        base: Box<SlotDisplay<M>>,
        material: Box<SlotDisplay<M>>,
        pattern: TrimPatternRef<M>,
    },
    WithRemainder {
        input: Box<SlotDisplay<M>>,
        remainder: Box<SlotDisplay<M>>,
    },
    Composite {
        contents: Vec<SlotDisplay<M>>,
    },
}

impl<M: Refs> SlotDisplay<M> {
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

impl crate::Encode for SlotDisplay<Wire> {
    fn encode(&self, mut w: impl Write) -> Result<()> {
        let w: &mut dyn Write = &mut w;
        self.kind().encode(&mut *w)?;
        match self {
            Self::Empty | Self::AnyFuel => Ok(()),
            Self::WithAnyPotion { contents } => contents.encode(w),
            Self::OnlyWithComponent {
                contents,
                component,
            } => {
                contents.encode(&mut *w)?;
                component.encode(w)
            }
            Self::Item { item } => item.encode(w),
            Self::ItemStack { item } => item.encode(w),
            Self::Tag { tag } => tag.encode(w),
            Self::Dyed { dye, target } => {
                dye.encode(&mut *w)?;
                target.encode(w)
            }
            Self::SmithingTrim {
                base,
                material,
                pattern,
            } => {
                base.encode(&mut *w)?;
                material.encode(&mut *w)?;
                pattern.encode(w)
            }
            Self::WithRemainder { input, remainder } => {
                input.encode(&mut *w)?;
                remainder.encode(w)
            }
            Self::Composite { contents } => contents.encode(w),
        }
    }
}

impl crate::Decode<'_> for SlotDisplay<Wire> {
    fn decode(r: &mut &[u8]) -> Result<Self> {
        nested(|| {
            let boxed = |r: &mut &[u8]| SlotDisplay::decode(r).map(Box::new);
            Ok(match SlotDisplayType::decode(r)? {
                SlotDisplayType::Empty => Self::Empty,
                SlotDisplayType::AnyFuel => Self::AnyFuel,
                SlotDisplayType::WithAnyPotion => Self::WithAnyPotion {
                    contents: boxed(r)?,
                },
                SlotDisplayType::OnlyWithComponent => Self::OnlyWithComponent {
                    contents: boxed(r)?,
                    component: DataComponentType::decode(r)?,
                },
                SlotDisplayType::Item => Self::Item {
                    item: crate::Decode::decode(r)?,
                },
                SlotDisplayType::ItemStack => Self::ItemStack {
                    item: Template::decode(r)?,
                },
                SlotDisplayType::Tag => Self::Tag {
                    tag: crate::Decode::decode(r)?,
                },
                SlotDisplayType::Dyed => Self::Dyed {
                    dye: boxed(r)?,
                    target: boxed(r)?,
                },
                SlotDisplayType::SmithingTrim => Self::SmithingTrim {
                    base: boxed(r)?,
                    material: boxed(r)?,
                    pattern: crate::Decode::decode(r)?,
                },
                SlotDisplayType::WithRemainder => Self::WithRemainder {
                    input: boxed(r)?,
                    remainder: boxed(r)?,
                },
                SlotDisplayType::Composite => Self::Composite {
                    contents: Vec::decode(r)?,
                },
            })
        })
    }
}

#[derive(Clone, Debug, PartialEq, FromWire)]
pub enum RecipeDisplay<M: Refs = Resolved> {
    CraftingShapeless {
        ingredients: Vec<SlotDisplay<M>>,
        result: SlotDisplay<M>,
        crafting_station: SlotDisplay<M>,
    },
    CraftingShaped {
        width: i32,
        height: i32,
        ingredients: Vec<SlotDisplay<M>>,
        result: SlotDisplay<M>,
        crafting_station: SlotDisplay<M>,
    },
    Furnace {
        ingredient: SlotDisplay<M>,
        fuel: SlotDisplay<M>,
        result: SlotDisplay<M>,
        crafting_station: SlotDisplay<M>,
        duration: i32,
        experience: f32,
    },
    Stonecutter {
        input: SlotDisplay<M>,
        result: SlotDisplay<M>,
        crafting_station: SlotDisplay<M>,
    },
    Smithing {
        template: SlotDisplay<M>,
        base: SlotDisplay<M>,
        addition: SlotDisplay<M>,
        result: SlotDisplay<M>,
        crafting_station: SlotDisplay<M>,
    },
}

impl<M: Refs> RecipeDisplay<M> {
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

impl<M: Refs> Validate for RecipeDisplay<M> {
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

impl crate::Encode for RecipeDisplay<Wire> {
    fn encode(&self, mut w: impl Write) -> Result<()> {
        self.validate().map_err(crate::Error::invalid)?;
        self.kind().encode(&mut w)?;
        match self {
            Self::CraftingShapeless {
                ingredients,
                result,
                crafting_station,
            } => {
                ingredients.encode(&mut w)?;
                result.encode(&mut w)?;
                crafting_station.encode(w)
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
                ingredients.encode(&mut w)?;
                result.encode(&mut w)?;
                crafting_station.encode(w)
            }
            Self::Furnace {
                ingredient,
                fuel,
                result,
                crafting_station,
                duration,
                experience,
            } => {
                ingredient.encode(&mut w)?;
                fuel.encode(&mut w)?;
                result.encode(&mut w)?;
                crafting_station.encode(&mut w)?;
                VarInt(*duration).encode(&mut w)?;
                experience.encode(w)
            }
            Self::Stonecutter {
                input,
                result,
                crafting_station,
            } => {
                input.encode(&mut w)?;
                result.encode(&mut w)?;
                crafting_station.encode(w)
            }
            Self::Smithing {
                template,
                base,
                addition,
                result,
                crafting_station,
            } => {
                template.encode(&mut w)?;
                base.encode(&mut w)?;
                addition.encode(&mut w)?;
                result.encode(&mut w)?;
                crafting_station.encode(w)
            }
        }
    }
}

impl crate::Decode<'_> for RecipeDisplay<Wire> {
    fn decode(r: &mut &[u8]) -> Result<Self> {
        let slot = |r: &mut &[u8]| SlotDisplay::decode(r);
        let display = match RecipeDisplayType::decode(r)? {
            RecipeDisplayType::CraftingShapeless => Self::CraftingShapeless {
                ingredients: Vec::decode(r)?,
                result: slot(r)?,
                crafting_station: slot(r)?,
            },
            RecipeDisplayType::CraftingShaped => Self::CraftingShaped {
                width: VarInt::decode(r)?.0,
                height: VarInt::decode(r)?.0,
                ingredients: Vec::decode(r)?,
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
        display.validate().map_err(crate::Error::invalid)?;
        Ok(display)
    }
}

/// One recipe as the client's recipe book shows it; `group` is the index of
/// its group, `crafting_requirements` is absent for recipes the book cannot
/// place, and `flags` carries [`Self::FLAG_NOTIFICATION`] and
/// [`Self::FLAG_HIGHLIGHT`].
#[derive(Clone, Debug, PartialEq, FromWire)]
pub struct RecipeBookEntry<M: Refs = Resolved> {
    pub id: VarInt,
    pub display: RecipeDisplay<M>,
    pub group: Option<u32>,
    pub category: RecipeBookCategory,
    pub crafting_requirements: Option<Vec<Ingredient<M>>>,
    pub flags: u8,
}

impl RecipeBookEntry {
    pub const FLAG_NOTIFICATION: u8 = 1;
    pub const FLAG_HIGHLIGHT: u8 = 2;
}

impl crate::Encode for RecipeBookEntry<Wire> {
    fn encode(&self, mut w: impl Write) -> Result<()> {
        self.id.encode(&mut w)?;
        self.display.encode(&mut w)?;
        OptionalUnsignedInt(self.group).encode(&mut w)?;
        self.category.encode(&mut w)?;
        self.crafting_requirements.encode(&mut w)?;
        self.flags.encode(w)
    }
}

impl crate::Decode<'_> for RecipeBookEntry<Wire> {
    fn decode(r: &mut &[u8]) -> Result<Self> {
        Ok(RecipeBookEntry {
            id: VarInt::decode(r)?,
            display: RecipeDisplay::decode(r)?,
            group: OptionalUnsignedInt::decode(r)?.0,
            category: RecipeBookCategory::decode(r)?,
            crafting_requirements: Option::decode(r)?,
            flags: u8::decode(r)?,
        })
    }
}

/// The items a `recipe_property_set` accepts, in the order vanilla's set
/// iterates them.
pub type RecipePropertySet<M = Resolved> = Vec<<M as Refs>::Id<Item>>;

/// One stonecutter option: the input it accepts and what the button shows.
#[derive(Clone, Debug, PartialEq, FromWire, Encode, Decode)]
pub struct SelectableRecipe<M: Refs = Resolved> {
    pub input: Ingredient<M>,
    pub option_display: SlotDisplay<M>,
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
