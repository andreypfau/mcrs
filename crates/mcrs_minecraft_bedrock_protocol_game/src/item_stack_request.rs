use alloc::vec::Vec;

use mcrs_minecraft_bedrock_wire::{
    BedrockDecode, BedrockEncode, Error, VarI32, VarU32, decode_counted_list, encode_counted_list,
};

use crate::item_descriptor::Ingredient;
use crate::item_stack::{ItemStack, decode_item, encode_item};

/// A request to change item stacks, the part of input flag 36 of
/// [`PlayerAuthInput`](crate::PlayerAuthInput).
#[derive(Clone, Debug, PartialEq, Eq, BedrockEncode, BedrockDecode)]
pub struct ItemStackRequest<'a> {
    /// The request id.
    pub request_id: VarI32,
    /// The actions, in order.
    pub actions: Vec<ItemStackAction<'a>>,
    /// The strings that the text filter checks, for example the new name of an item in an anvil.
    pub strings_to_filter: Vec<&'a str>,
    /// The origin of the filtered strings.
    pub filter_origin: i32,
}

/// A slot reference of an [`ItemStackAction`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, BedrockEncode, BedrockDecode)]
pub struct StackSlot {
    /// The container.
    pub container: u8,
    /// The dynamic container id, when the container has one.
    pub dynamic_container_id: Option<u32>,
    /// The slot.
    pub slot: u8,
    /// The stack network id of the item in the slot.
    pub stack_network_id: VarI32,
}

/// One action of an [`ItemStackRequest`]; an unknown type is a decode error.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ItemStackAction<'a> {
    /// Type 0: takes items.
    Take {
        /// The number of items.
        count: u8,
        /// The source slot.
        source: StackSlot,
        /// The destination slot.
        destination: StackSlot,
    },
    /// Type 1: places items.
    Place {
        /// The number of items.
        count: u8,
        /// The source slot.
        source: StackSlot,
        /// The destination slot.
        destination: StackSlot,
    },
    /// Type 2: swaps two stacks.
    Swap {
        /// The source slot.
        source: StackSlot,
        /// The destination slot.
        destination: StackSlot,
    },
    /// Type 3: drops items.
    Drop {
        /// The number of items.
        count: u8,
        /// The source slot.
        source: StackSlot,
        /// Whether the items drop in a random direction.
        randomly: bool,
    },
    /// Type 4: destroys items.
    Destroy {
        /// The number of items.
        count: u8,
        /// The source slot.
        source: StackSlot,
    },
    /// Type 5: consumes items.
    Consume {
        /// The number of items.
        count: u8,
        /// The source slot.
        source: StackSlot,
    },
    /// Type 6: creates the result of a craft.
    Create {
        /// The result slot.
        result_slot: u8,
    },
    /// Type 7: places items in an item container.
    PlaceInItemContainer {
        /// The number of items.
        count: u8,
        /// The source slot.
        source: StackSlot,
        /// The destination slot.
        destination: StackSlot,
    },
    /// Type 8: takes items from an item container.
    TakeFromItemContainer {
        /// The number of items.
        count: u8,
        /// The source slot.
        source: StackSlot,
        /// The destination slot.
        destination: StackSlot,
    },
    /// Type 9: combines in a lab table.
    LabTableCombine,
    /// Type 10: pays a beacon.
    BeaconPayment {
        /// The primary effect.
        primary_effect: VarI32,
        /// The secondary effect.
        secondary_effect: VarI32,
    },
    /// Type 11: mines a block.
    MineBlock {
        /// The hotbar slot.
        hotbar_slot: VarI32,
        /// The predicted durability.
        predicted_durability: VarI32,
        /// The stack network id.
        stack_network_id: VarI32,
    },
    /// Type 12: crafts a recipe.
    CraftRecipe {
        /// The recipe network id.
        recipe_network_id: VarU32,
        /// The number of crafts.
        number_of_crafts: u8,
    },
    /// Type 13: crafts a recipe from the recipe book.
    AutoCraftRecipe {
        /// The recipe network id.
        recipe_network_id: VarU32,
        /// The number of crafts.
        number_of_crafts: u8,
        /// The times crafted.
        times_crafted: u8,
        /// The ingredients, behind a `u8` count.
        ingredients: Vec<Ingredient<'a>>,
    },
    /// Type 14: takes a creative item.
    CraftCreative {
        /// The creative item network id.
        creative_item_network_id: VarU32,
        /// The number of crafts.
        number_of_crafts: u8,
    },
    /// Type 15: crafts a recipe with a filtered string, for example in an anvil.
    CraftRecipeOptional {
        /// The recipe network id.
        recipe_network_id: VarU32,
        /// The index in [`ItemStackRequest::strings_to_filter`].
        filter_string_index: i32,
    },
    /// Type 16: crafts in a grindstone.
    CraftGrindstone {
        /// The recipe network id.
        recipe_network_id: VarU32,
        /// The number of crafts.
        number_of_crafts: u8,
        /// The repair cost.
        repair_cost: VarI32,
    },
    /// Type 17: crafts in a loom.
    CraftLoom {
        /// The pattern id.
        pattern_id: &'a str,
        /// The times crafted.
        times_crafted: u8,
    },
    /// Type 18: a craft that the client does not implement.
    CraftNonImplemented,
    /// Type 19, no longer used: the results of a craft.
    CraftResults {
        /// The result items; `None` is the empty item.
        result_items: Vec<Option<ItemStack<'a>>>,
        /// The times crafted.
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
