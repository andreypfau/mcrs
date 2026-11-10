use alloc::borrow::Cow;
use alloc::vec::Vec;
use core::num::NonZeroU8;

use mcrs_minecraft_resource::ResourceLocation;
use mcrs_minecraft_text::Text;
use mcrs_minecraft_wire::Decode;
use mcrs_minecraft_wire::Encode;
use mcrs_minecraft_wire::VarInt;

use crate::AttributeId;
use crate::AttributeModifier;
use crate::AttributeModifiers;
use crate::BlockId;
use crate::BlockSet;
use crate::BlockTagId;
use crate::Builtin;
use crate::BuiltinKey;
use crate::ComponentType;
use crate::CustomName;
use crate::Damage;
use crate::EnchantmentId;
use crate::EnchantmentLevel;
use crate::Enchantments;
use crate::Food;
use crate::ItemModel;
use crate::ItemName;
use crate::Lore;
use crate::MaxDamage;
use crate::MaxStackSize;
use crate::ModifierDisplay;
use crate::ModifierOperation;
use crate::NameLookup;
use crate::NameTable;
use crate::Rarity;
use crate::RepairCost;
use crate::SlotGroup;
use crate::Tool;
use crate::ToolRule;
use crate::TooltipDisplay;
use crate::Unbreakable;
use crate::WireError;
use crate::lore::MAX_LINES;
use crate::wire_text::read_text;
use crate::wire_text::write_text;

const RARITIES: [Rarity; 4] = [Rarity::Common, Rarity::Uncommon, Rarity::Rare, Rarity::Epic];

const OPERATIONS: [ModifierOperation; 3] = [
    ModifierOperation::AddValue,
    ModifierOperation::AddMultipliedBase,
    ModifierOperation::AddMultipliedTotal,
];

const SLOT_GROUPS: [SlotGroup; 11] = [
    SlotGroup::Any,
    SlotGroup::Mainhand,
    SlotGroup::Offhand,
    SlotGroup::Hand,
    SlotGroup::Feet,
    SlotGroup::Legs,
    SlotGroup::Chest,
    SlotGroup::Head,
    SlotGroup::Armor,
    SlotGroup::Body,
    SlotGroup::Saddle,
];

const DISPLAY_DEFAULT: i32 = 0;
const DISPLAY_HIDDEN: i32 = 1;
const DISPLAY_OVERRIDE: i32 = 2;

const BLOCK_TAG: i32 = 0;

/// Appends `value` as a VarInt.
pub(crate) fn put_var_int(output: &mut Vec<u8>, value: i32) {
    let mut rest = value.cast_unsigned();
    while rest >= 0x80 {
        output.push((rest & 0x7f) as u8 | 0x80);
        rest >>= 7;
    }
    output.push(rest as u8);
}

pub(crate) fn put_length(output: &mut Vec<u8>, length: usize) -> Result<(), WireError> {
    put_var_int(
        output,
        i32::try_from(length).map_err(|_| WireError::TooLong)?,
    );
    Ok(())
}

/// Reads the wire form of one component.
pub(crate) fn read_value<L: NameLookup>(
    component: BuiltinKey,
    input: &mut &[u8],
    names: &L,
) -> Result<Builtin, WireError> {
    let reader = Reader { component, names };
    Ok(match component {
        BuiltinKey::MaxStackSize => Builtin::MaxStackSize(MaxStackSize(reader.var_int(input)?)),
        BuiltinKey::MaxDamage => Builtin::MaxDamage(MaxDamage(reader.var_int(input)?)),
        BuiltinKey::Damage => Builtin::Damage(Damage(reader.var_int(input)?)),
        BuiltinKey::RepairCost => Builtin::RepairCost(RepairCost(reader.var_int(input)?)),
        BuiltinKey::Unbreakable => Builtin::Unbreakable(Unbreakable),
        BuiltinKey::ItemModel => {
            Builtin::ItemModel(ItemModel::new(reader.identifier(input)?.to_buf()))
        }
        BuiltinKey::Rarity => Builtin::Rarity(reader.list_entry(input, &RARITIES)?),
        BuiltinKey::Enchantments => Builtin::Enchantments(reader.enchantments(input)?),
        BuiltinKey::AttributeModifiers => {
            Builtin::AttributeModifiers(reader.attribute_modifiers(input)?)
        }
        BuiltinKey::TooltipDisplay => Builtin::TooltipDisplay(reader.tooltip_display(input)?),
        BuiltinKey::Food => Builtin::Food(Food {
            nutrition: reader.var_int(input)?,
            saturation: f32::from_be_bytes(reader.bytes(input)?),
            can_always_eat: reader.boolean(input)?,
        }),
        BuiltinKey::Tool => Builtin::Tool(reader.tool(input)?),
        BuiltinKey::CustomName => Builtin::CustomName(CustomName(read_text(input, component)?)),
        BuiltinKey::ItemName => Builtin::ItemName(ItemName(read_text(input, component)?)),
        BuiltinKey::Lore => Builtin::Lore(reader.lore(input)?),
    })
}

struct Reader<'names, L> {
    component: BuiltinKey,
    names: &'names L,
}

impl<L: NameLookup> Reader<'_, L> {
    fn bad(&self) -> WireError {
        WireError::BadValue(self.component)
    }

    fn var_int(&self, input: &mut &[u8]) -> Result<i32, WireError> {
        VarInt::decode(input)
            .map(|VarInt(value)| value)
            .map_err(|error| WireError::of_value(&error, self.component))
    }

    fn length(&self, input: &mut &[u8]) -> Result<usize, WireError> {
        usize::try_from(self.var_int(input)?).map_err(|_| self.bad())
    }

    fn id(&self, input: &mut &[u8], table: NameTable) -> Result<u16, WireError> {
        u16::try_from(self.var_int(input)?).map_err(|_| WireError::UnknownEntry {
            component: self.component,
            table,
        })
    }

    fn list_entry<T: Copy>(&self, input: &mut &[u8], list: &[T]) -> Result<T, WireError> {
        let id = self.var_int(input)?;
        usize::try_from(id)
            .ok()
            .and_then(|index| list.get(index).copied())
            .ok_or(self.bad())
    }

    fn bytes<const N: usize>(&self, input: &mut &[u8]) -> Result<[u8; N], WireError> {
        let (bytes, rest) = input
            .split_first_chunk::<N>()
            .ok_or(WireError::UnexpectedEnd)?;
        *input = rest;
        Ok(*bytes)
    }

    fn boolean(&self, input: &mut &[u8]) -> Result<bool, WireError> {
        match self.bytes(input)? {
            [0] => Ok(false),
            [1] => Ok(true),
            _ => Err(self.bad()),
        }
    }

    fn identifier<'input>(
        &self,
        input: &mut &'input [u8],
    ) -> Result<ResourceLocation<'input>, WireError> {
        ResourceLocation::decode(input).map_err(|error| WireError::of_value(&error, self.component))
    }

    fn enchantments(&self, input: &mut &[u8]) -> Result<Enchantments, WireError> {
        let count = self.length(input)?;
        let mut entries: Vec<EnchantmentLevel> = Vec::with_capacity(count.min(input.len()));
        for _ in 0..count {
            let enchantment = EnchantmentId(self.id(input, NameTable::Enchantment)?);
            let level = u8::try_from(self.var_int(input)?).map_err(|_| self.bad())?;
            let level = NonZeroU8::new(level).ok_or(WireError::OutOfRange(self.component))?;
            match entries
                .iter_mut()
                .find(|entry| entry.enchantment == enchantment)
            {
                Some(entry) => entry.level = level,
                None => entries.push(EnchantmentLevel { enchantment, level }),
            }
        }
        Enchantments::new(entries).ok_or(self.bad())
    }

    fn lore(&self, input: &mut &[u8]) -> Result<Lore, WireError> {
        let count = self.length(input)?;
        if count > MAX_LINES {
            return Err(WireError::OutOfRange(self.component));
        }
        let mut lines: Vec<Text> = Vec::with_capacity(count);
        for _ in 0..count {
            lines.push(read_text(input, self.component)?);
        }
        Ok(Lore {
            lines: Cow::Owned(lines),
        })
    }

    fn attribute_modifiers(&self, input: &mut &[u8]) -> Result<AttributeModifiers, WireError> {
        let count = self.length(input)?;
        let mut modifiers: Vec<AttributeModifier> = Vec::with_capacity(count.min(input.len()));
        for _ in 0..count {
            let attribute = AttributeId(self.id(input, NameTable::Attribute)?);
            let id = self.identifier(input)?.to_buf();
            let amount = f64::from_be_bytes(self.bytes(input)?);
            let operation = self.list_entry(input, &OPERATIONS)?;
            let mut modifier = AttributeModifier::new(attribute, id, amount, operation);
            modifier.slot = self.list_entry(input, &SLOT_GROUPS)?;
            modifier.display = match self.var_int(input)? {
                DISPLAY_DEFAULT => ModifierDisplay::Default,
                DISPLAY_HIDDEN => ModifierDisplay::Hidden,
                DISPLAY_OVERRIDE => ModifierDisplay::Override(read_text(input, self.component)?),
                _ => return Err(self.bad()),
            };
            modifiers.push(modifier);
        }
        Ok(AttributeModifiers {
            modifiers: Cow::Owned(modifiers),
        })
    }

    fn tooltip_display(&self, input: &mut &[u8]) -> Result<TooltipDisplay, WireError> {
        let hide_tooltip = self.boolean(input)?;
        let count = self.length(input)?;
        let mut hidden: Vec<ComponentType> = Vec::with_capacity(count.min(input.len()));
        for _ in 0..count {
            hidden.push(ComponentType(self.id(input, NameTable::ComponentType)?));
        }
        let mut display = TooltipDisplay::hiding(hidden);
        display.hide_tooltip = hide_tooltip;
        Ok(display)
    }

    fn tool(&self, input: &mut &[u8]) -> Result<Tool, WireError> {
        let count = self.length(input)?;
        let mut rules: Vec<ToolRule> = Vec::with_capacity(count.min(input.len()));
        for _ in 0..count {
            rules.push(self.tool_rule(input)?);
        }
        Ok(Tool {
            rules: Cow::Owned(rules),
            default_mining_speed: f32::from_be_bytes(self.bytes(input)?),
            damage_per_block: self.var_int(input)?,
            can_destroy_blocks_in_creative: self.boolean(input)?,
        })
    }

    fn tool_rule(&self, input: &mut &[u8]) -> Result<ToolRule, WireError> {
        let mut rule = ToolRule::new(self.block_set(input)?);
        if self.boolean(input)? {
            rule = rule
                .with_speed(f32::from_be_bytes(self.bytes(input)?))
                .ok_or(WireError::OutOfRange(self.component))?;
        }
        if self.boolean(input)? {
            rule.correct_for_drops = Some(self.boolean(input)?);
        }
        Ok(rule)
    }

    fn block_set(&self, input: &mut &[u8]) -> Result<BlockSet, WireError> {
        let form = self.var_int(input)?;
        if form == BLOCK_TAG {
            let name = self.identifier(input)?;
            let block_tag =
                self.names
                    .id(NameTable::BlockTag, name)
                    .ok_or(WireError::UnknownEntry {
                        component: self.component,
                        table: NameTable::BlockTag,
                    })?;
            return Ok(BlockSet::Tag(BlockTagId(block_tag)));
        }
        let count = usize::try_from(form)
            .ok()
            .and_then(|form| form.checked_sub(1))
            .ok_or(self.bad())?;
        let mut blocks: Vec<BlockId> = Vec::with_capacity(count.min(input.len()));
        for _ in 0..count {
            blocks.push(BlockId(self.id(input, NameTable::Block)?));
        }
        Ok(BlockSet::Blocks(Cow::Owned(blocks)))
    }
}

/// Appends the wire form of one component.
pub(crate) fn write_value<L: NameLookup>(
    value: &Builtin,
    names: &L,
    output: &mut Vec<u8>,
) -> Result<(), WireError> {
    let writer = Writer {
        component: value.key(),
        names,
    };
    match value {
        Builtin::MaxStackSize(MaxStackSize(number))
        | Builtin::MaxDamage(MaxDamage(number))
        | Builtin::Damage(Damage(number))
        | Builtin::RepairCost(RepairCost(number)) => put_var_int(output, *number),
        Builtin::Unbreakable(Unbreakable) => {}
        Builtin::ItemModel(model) => writer.identifier(model.get(), output)?,
        Builtin::Rarity(rarity) => writer.list_id(&RARITIES, *rarity, output)?,
        Builtin::Enchantments(enchantments) => {
            put_length(output, enchantments.entries().len())?;
            for entry in enchantments.entries() {
                put_var_int(output, i32::from(entry.enchantment.0));
                put_var_int(output, i32::from(entry.level.get()));
            }
        }
        Builtin::AttributeModifiers(modifiers) => writer.attribute_modifiers(modifiers, output)?,
        Builtin::TooltipDisplay(display) => {
            put_boolean(output, display.hide_tooltip);
            put_length(output, display.hidden_components().len())?;
            for component_type in display.hidden_components() {
                put_var_int(output, i32::from(component_type.0));
            }
        }
        Builtin::Food(food) => {
            put_var_int(output, food.nutrition);
            output.extend_from_slice(&food.saturation.to_be_bytes());
            put_boolean(output, food.can_always_eat);
        }
        Builtin::Tool(tool) => writer.tool(tool, output)?,
        Builtin::CustomName(CustomName(text)) | Builtin::ItemName(ItemName(text)) => {
            write_text(text, writer.component, output)?;
        }
        Builtin::Lore(lore) => {
            if lore.lines.len() > MAX_LINES {
                return Err(WireError::OutOfRange(writer.component));
            }
            put_length(output, lore.lines.len())?;
            for line in lore.lines.iter() {
                write_text(line, writer.component, output)?;
            }
        }
    }
    Ok(())
}

fn put_boolean(output: &mut Vec<u8>, value: bool) {
    output.push(u8::from(value));
}

struct Writer<'names, L> {
    component: BuiltinKey,
    names: &'names L,
}

impl<L: NameLookup> Writer<'_, L> {
    fn identifier(
        &self,
        location: ResourceLocation<'_>,
        output: &mut Vec<u8>,
    ) -> Result<(), WireError> {
        location
            .encode(&mut *output)
            .map_err(|_| WireError::OutOfRange(self.component))
    }

    fn list_id<T: PartialEq>(
        &self,
        list: &[T],
        value: T,
        output: &mut Vec<u8>,
    ) -> Result<(), WireError> {
        let index = list
            .iter()
            .position(|entry| *entry == value)
            .ok_or(WireError::BadValue(self.component))?;
        put_length(output, index)
    }

    fn attribute_modifiers(
        &self,
        modifiers: &AttributeModifiers,
        output: &mut Vec<u8>,
    ) -> Result<(), WireError> {
        put_length(output, modifiers.modifiers.len())?;
        for modifier in modifiers.modifiers.iter() {
            put_var_int(output, i32::from(modifier.attribute.0));
            self.identifier(modifier.id(), output)?;
            output.extend_from_slice(&modifier.amount.to_be_bytes());
            self.list_id(&OPERATIONS, modifier.operation, output)?;
            self.list_id(&SLOT_GROUPS, modifier.slot, output)?;
            match &modifier.display {
                ModifierDisplay::Default => put_var_int(output, DISPLAY_DEFAULT),
                ModifierDisplay::Hidden => put_var_int(output, DISPLAY_HIDDEN),
                ModifierDisplay::Override(text) => {
                    put_var_int(output, DISPLAY_OVERRIDE);
                    write_text(text, self.component, output)?;
                }
            }
        }
        Ok(())
    }

    fn tool(&self, tool: &Tool, output: &mut Vec<u8>) -> Result<(), WireError> {
        put_length(output, tool.rules.len())?;
        for rule in tool.rules.iter() {
            self.block_set(&rule.blocks, output)?;
            put_boolean(output, rule.speed().is_some());
            if let Some(speed) = rule.speed() {
                output.extend_from_slice(&speed.to_be_bytes());
            }
            put_boolean(output, rule.correct_for_drops.is_some());
            if let Some(correct_for_drops) = rule.correct_for_drops {
                put_boolean(output, correct_for_drops);
            }
        }
        output.extend_from_slice(&tool.default_mining_speed.to_be_bytes());
        put_var_int(output, tool.damage_per_block);
        put_boolean(output, tool.can_destroy_blocks_in_creative);
        Ok(())
    }

    fn block_set(&self, blocks: &BlockSet, output: &mut Vec<u8>) -> Result<(), WireError> {
        match blocks {
            BlockSet::Tag(block_tag) => {
                let name = self
                    .names
                    .name(NameTable::BlockTag, block_tag.0)
                    .ok_or(WireError::UnnamedId(NameTable::BlockTag))?;
                put_var_int(output, BLOCK_TAG);
                self.identifier(name, output)
            }
            BlockSet::Blocks(blocks) => {
                let form = blocks.len().checked_add(1).ok_or(WireError::TooLong)?;
                put_length(output, form)?;
                for block in blocks.iter() {
                    put_var_int(output, i32::from(block.0));
                }
                Ok(())
            }
        }
    }
}
