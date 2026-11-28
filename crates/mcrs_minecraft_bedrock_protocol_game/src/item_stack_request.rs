use alloc::vec::Vec;

use mcrs_minecraft_bedrock_wire::{
    BedrockDecode, BedrockEncode, Error, VarI32, VarU32, decode_counted_list, encode_counted_list,
};

use crate::item_descriptor::Ingredient;
use crate::item_stack::{ItemStack, decode_item, encode_item};

#[derive(Clone, Debug, PartialEq, Eq, BedrockEncode, BedrockDecode)]
pub struct ItemStackRequest<'a> {
    pub request_id: VarI32,
    pub actions: Vec<ItemStackAction<'a>>,
    pub strings_to_filter: Vec<&'a str>,
    pub filter_origin: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, BedrockEncode, BedrockDecode)]
pub struct StackSlot {
    pub container: u8,
    pub dynamic_container_id: Option<u32>,
    pub slot: u8,
    pub stack_network_id: VarI32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ItemStackAction<'a> {
    Take {
        count: u8,
        source: StackSlot,
        destination: StackSlot,
    },
    Place {
        count: u8,
        source: StackSlot,
        destination: StackSlot,
    },
    Swap {
        source: StackSlot,
        destination: StackSlot,
    },
    Drop {
        count: u8,
        source: StackSlot,
        randomly: bool,
    },
    Destroy {
        count: u8,
        source: StackSlot,
    },
    Consume {
        count: u8,
        source: StackSlot,
    },
    Create {
        result_slot: u8,
    },
    PlaceInItemContainer {
        count: u8,
        source: StackSlot,
        destination: StackSlot,
    },
    TakeFromItemContainer {
        count: u8,
        source: StackSlot,
        destination: StackSlot,
    },
    LabTableCombine,
    BeaconPayment {
        primary_effect: VarI32,
        secondary_effect: VarI32,
    },
    MineBlock {
        hotbar_slot: VarI32,
        predicted_durability: VarI32,
        stack_network_id: VarI32,
    },
    CraftRecipe {
        recipe_network_id: VarU32,
        number_of_crafts: u8,
    },
    AutoCraftRecipe {
        recipe_network_id: VarU32,
        number_of_crafts: u8,
        times_crafted: u8,
        ingredients: Vec<Ingredient<'a>>,
    },
    CraftCreative {
        creative_item_network_id: VarU32,
        number_of_crafts: u8,
    },
    CraftRecipeOptional {
        recipe_network_id: VarU32,
        filter_string_index: i32,
    },
    CraftGrindstone {
        recipe_network_id: VarU32,
        number_of_crafts: u8,
        repair_cost: VarI32,
    },
    CraftLoom {
        pattern_id: &'a str,
        times_crafted: u8,
    },
    CraftNonImplemented,
    CraftResults {
        result_items: Vec<Option<ItemStack<'a>>>,
        times_crafted: u8,
    },
}

impl ItemStackAction<'_> {
    const fn wire_type(&self) -> u8 {
        match self {
            Self::Take { .. } => 0,
            Self::Place { .. } => 1,
            Self::Swap { .. } => 2,
            Self::Drop { .. } => 3,
            Self::Destroy { .. } => 4,
            Self::Consume { .. } => 5,
            Self::Create { .. } => 6,
            Self::PlaceInItemContainer { .. } => 7,
            Self::TakeFromItemContainer { .. } => 8,
            Self::LabTableCombine => 9,
            Self::BeaconPayment { .. } => 10,
            Self::MineBlock { .. } => 11,
            Self::CraftRecipe { .. } => 12,
            Self::AutoCraftRecipe { .. } => 13,
            Self::CraftCreative { .. } => 14,
            Self::CraftRecipeOptional { .. } => 15,
            Self::CraftGrindstone { .. } => 16,
            Self::CraftLoom { .. } => 17,
            Self::CraftNonImplemented => 18,
            Self::CraftResults { .. } => 19,
        }
    }
}

impl BedrockEncode for ItemStackAction<'_> {
    #[expect(
        clippy::too_many_lines,
        reason = "one flat match over the twenty action types, in the order of the specification"
    )]
    fn encode(&self, w: &mut Vec<u8>) -> Result<(), Error> {
        use ItemStackAction::*;
        self.wire_type().encode(w)?;
        match self {
            Take {
                count,
                source,
                destination,
            }
            | Place {
                count,
                source,
                destination,
            }
            | PlaceInItemContainer {
                count,
                source,
                destination,
            }
            | TakeFromItemContainer {
                count,
                source,
                destination,
            } => {
                count.encode(w)?;
                source.encode(w)?;
                destination.encode(w)
            }
            Swap {
                source,
                destination,
            } => {
                source.encode(w)?;
                destination.encode(w)
            }
            Drop {
                count,
                source,
                randomly,
            } => {
                count.encode(w)?;
                source.encode(w)?;
                randomly.encode(w)
            }
            Destroy { count, source } | Consume { count, source } => {
                count.encode(w)?;
                source.encode(w)
            }
            Create { result_slot } => result_slot.encode(w),
            LabTableCombine | CraftNonImplemented => Ok(()),
            BeaconPayment {
                primary_effect,
                secondary_effect,
            } => {
                primary_effect.encode(w)?;
                secondary_effect.encode(w)
            }
            MineBlock {
                hotbar_slot,
                predicted_durability,
                stack_network_id,
            } => {
                hotbar_slot.encode(w)?;
                predicted_durability.encode(w)?;
                stack_network_id.encode(w)
            }
            CraftRecipe {
                recipe_network_id,
                number_of_crafts,
            }
            | CraftCreative {
                creative_item_network_id: recipe_network_id,
                number_of_crafts,
            } => {
                recipe_network_id.encode(w)?;
                number_of_crafts.encode(w)
            }
            AutoCraftRecipe {
                recipe_network_id,
                number_of_crafts,
                times_crafted,
                ingredients,
            } => {
                recipe_network_id.encode(w)?;
                number_of_crafts.encode(w)?;
                times_crafted.encode(w)?;
                encode_counted_list::<u8, _>(ingredients, w)
            }
            CraftRecipeOptional {
                recipe_network_id,
                filter_string_index,
            } => {
                recipe_network_id.encode(w)?;
                filter_string_index.encode(w)
            }
            CraftGrindstone {
                recipe_network_id,
                number_of_crafts,
                repair_cost,
            } => {
                recipe_network_id.encode(w)?;
                number_of_crafts.encode(w)?;
                repair_cost.encode(w)
            }
            CraftLoom {
                pattern_id,
                times_crafted,
            } => {
                pattern_id.encode(w)?;
                times_crafted.encode(w)
            }
            CraftResults {
                result_items,
                times_crafted,
            } => {
                let count = u32::try_from(result_items.len()).map_err(|_| Error::LengthOverflow)?;
                VarU32(count).encode(w)?;
                for item in result_items {
                    encode_item(item.as_ref(), w)?;
                }
                times_crafted.encode(w)
            }
        }
    }
}

impl<'a> BedrockDecode<'a> for ItemStackAction<'a> {
    fn decode(r: &mut &'a [u8]) -> Result<Self, Error> {
        use ItemStackAction::*;
        Ok(match u8::decode(r)? {
            0 => Take {
                count: u8::decode(r)?,
                source: StackSlot::decode(r)?,
                destination: StackSlot::decode(r)?,
            },
            1 => Place {
                count: u8::decode(r)?,
                source: StackSlot::decode(r)?,
                destination: StackSlot::decode(r)?,
            },
            2 => Swap {
                source: StackSlot::decode(r)?,
                destination: StackSlot::decode(r)?,
            },
            3 => Drop {
                count: u8::decode(r)?,
                source: StackSlot::decode(r)?,
                randomly: bool::decode(r)?,
            },
            4 => Destroy {
                count: u8::decode(r)?,
                source: StackSlot::decode(r)?,
            },
            5 => Consume {
                count: u8::decode(r)?,
                source: StackSlot::decode(r)?,
            },
            6 => Create {
                result_slot: u8::decode(r)?,
            },
            7 => PlaceInItemContainer {
                count: u8::decode(r)?,
                source: StackSlot::decode(r)?,
                destination: StackSlot::decode(r)?,
            },
            8 => TakeFromItemContainer {
                count: u8::decode(r)?,
                source: StackSlot::decode(r)?,
                destination: StackSlot::decode(r)?,
            },
            9 => LabTableCombine,
            10 => BeaconPayment {
                primary_effect: VarI32::decode(r)?,
                secondary_effect: VarI32::decode(r)?,
            },
            11 => MineBlock {
                hotbar_slot: VarI32::decode(r)?,
                predicted_durability: VarI32::decode(r)?,
                stack_network_id: VarI32::decode(r)?,
            },
            12 => CraftRecipe {
                recipe_network_id: VarU32::decode(r)?,
                number_of_crafts: u8::decode(r)?,
            },
            13 => AutoCraftRecipe {
                recipe_network_id: VarU32::decode(r)?,
                number_of_crafts: u8::decode(r)?,
                times_crafted: u8::decode(r)?,
                ingredients: decode_counted_list::<u8, _>(r)?,
            },
            14 => CraftCreative {
                creative_item_network_id: VarU32::decode(r)?,
                number_of_crafts: u8::decode(r)?,
            },
            15 => CraftRecipeOptional {
                recipe_network_id: VarU32::decode(r)?,
                filter_string_index: i32::decode(r)?,
            },
            16 => CraftGrindstone {
                recipe_network_id: VarU32::decode(r)?,
                number_of_crafts: u8::decode(r)?,
                repair_cost: VarI32::decode(r)?,
            },
            17 => CraftLoom {
                pattern_id: BedrockDecode::decode(r)?,
                times_crafted: u8::decode(r)?,
            },
            18 => CraftNonImplemented,
            19 => CraftResults {
                result_items: decode_result_items(r)?,
                times_crafted: u8::decode(r)?,
            },
            other => return Err(Error::UnknownEnumValue(i64::from(other))),
        })
    }
}

fn decode_result_items<'a>(r: &mut &'a [u8]) -> Result<Vec<Option<ItemStack<'a>>>, Error> {
    let VarU32(count) = VarU32::decode(r)?;
    let count = usize::try_from(count).map_err(|_| Error::LengthOverflow)?;
    (0..count).map(|_| decode_item(r)).collect()
}
