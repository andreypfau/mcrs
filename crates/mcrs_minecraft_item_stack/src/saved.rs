use alloc::borrow::Cow;
use alloc::boxed::Box;
use alloc::format;
use alloc::string::ToString;
use alloc::vec::Vec;
use core::num::NonZeroU8;
use core::ops::RangeInclusive;
use std::io::Cursor;

use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_nbt::deserializer::NbtReadHelper;
use mcrs_minecraft_nbt::serializer::WriteAdaptor;
use mcrs_minecraft_nbt::tag::NbtTag;
use mcrs_minecraft_resource::ResourceLocation;

use crate::AttributeId;
use crate::AttributeModifier;
use crate::AttributeModifiers;
use crate::BlockId;
use crate::BlockSet;
use crate::BlockTagId;
use crate::Builtin;
use crate::BuiltinKey;
use crate::ComponentType;
use crate::Count;
use crate::CustomName;
use crate::Damage;
use crate::EnchantmentId;
use crate::EnchantmentLevel;
use crate::Enchantments;
use crate::Food;
use crate::ItemModel;
use crate::ItemName;
use crate::ItemStack;
use crate::Kind;
use crate::MaxDamage;
use crate::MaxStackSize;
use crate::ModifierDisplay;
use crate::ModifierOperation;
use crate::NameLookup;
use crate::NameTable;
use crate::PatchEntry;
use crate::Rarity;
use crate::RepairCost;
use crate::SavedError;
use crate::SlotGroup;
use crate::SlotStack;
use crate::Tool;
use crate::ToolRule;
use crate::TooltipDisplay;
use crate::Unbreakable;
use crate::UndecodedValue;
use crate::builtin_key::NAMESPACE;
use crate::check_saved_form::MAX_COUNT;
use crate::check_saved_form::NOT_NEGATIVE;
use crate::check_saved_form::POSITIVE;
use crate::check_saved_form::STACK_SIZE;
use crate::saved_key::KeyKind;
use crate::saved_key::REMOVAL_MARKER;
use crate::saved_key::SavedKey;
use crate::saved_text::read_lore;
use crate::saved_text::read_text;
use crate::saved_text::write_lore;
use crate::saved_text::write_text;
use crate::tag_number::TagNumber;

const ID: &str = "id";
const COUNT: &str = "count";
const COMPONENTS: &str = "components";
const SLOT: &str = "Slot";
const AIR: &str = "air";
const TAG_MARKER: char = '#';

const DISPLAY_DEFAULT: &str = "default";
const DISPLAY_HIDDEN: &str = "hidden";
const DISPLAY_OVERRIDE: &str = "override";

const DEFAULT_MINING_SPEED: f32 = 1.0;
const DEFAULT_DAMAGE_PER_BLOCK: i32 = 1;

/// Reads the saved stack: a map with the item, the count and the component patch.
///
/// A component of a type without a Rust type in this crate becomes a [`PatchEntry::Undecoded`]. The reader has no partial result: one bad key of the
/// patch gives the error and no stack.
pub fn read_saved<L: NameLookup>(saved: &NbtCompound, names: &L) -> Result<ItemStack, SavedError> {
    let Some(NbtTag::String(id)) = saved.get(ID) else {
        return Err(SavedError::BadItemId);
    };
    let item = ResourceLocation::read(id).map_err(|_| SavedError::BadItemId)?;
    if is_air(item) {
        return Err(SavedError::AirItem);
    }
    let kind = names
        .id(NameTable::Item, item)
        .ok_or(SavedError::UnknownItem)?;
    let count = match saved.get(COUNT) {
        None => 1,
        Some(count) => TagNumber::of(count).ok_or(SavedError::BadCount)?.to_int(),
    };
    let count = u8::try_from(count)
        .ok()
        .filter(|count| *count <= MAX_COUNT)
        .and_then(Count::new)
        .ok_or(SavedError::BadCount)?;
    let mut stack = ItemStack::new(Kind(kind), count);
    match saved.get(COMPONENTS) {
        None => {}
        Some(NbtTag::Compound(patch)) => {
            for (index, (key, value)) in patch.child_tags.iter().enumerate() {
                let key = SavedKey::read(key)?;
                if key.is_repeated(patch.child_tags.get(..index).unwrap_or_default()) {
                    return Err(SavedError::RepeatedKey);
                }
                stack.put_entry(read_entry(key, value, names)?);
            }
        }
        Some(_) => return Err(SavedError::BadPatch),
    }
    Ok(stack)
}

/// Writes the saved stack. The result always has the item and the count, and has the patch when
/// the patch is not empty.
///
/// The writer omits a custom component, which has no saved form.
pub fn write_saved<L: NameLookup>(stack: &ItemStack, names: &L) -> Result<NbtCompound, SavedError> {
    let item = names
        .name(NameTable::Item, stack.kind.0)
        .ok_or(SavedError::UnnamedId(NameTable::Item))?;
    if is_air(item) {
        return Err(SavedError::AirItem);
    }
    let count = stack.count.get();
    if count > MAX_COUNT {
        return Err(SavedError::BadCount);
    }
    let mut saved = NbtCompound::new();
    saved.put(ID, NbtTag::String(item.to_string()));
    saved.put(COUNT, NbtTag::Int(i32::from(count)));
    let mut patch = NbtCompound::new();
    for entry in stack.patch() {
        write_entry(entry, names, &mut patch)?;
    }
    if !patch.is_empty() {
        saved.put(COMPONENTS, NbtTag::Compound(patch));
    }
    Ok(saved)
}

/// Reads the optional saved stack: a map without keys gives `None`, the empty stack.
pub fn read_saved_optional<L: NameLookup>(
    saved: &NbtCompound,
    names: &L,
) -> Result<Option<ItemStack>, SavedError> {
    if saved.is_empty() {
        Ok(None)
    } else {
        read_saved(saved, names).map(Some)
    }
}

/// Writes the optional saved stack: `None`, the empty stack, gives a map without keys.
pub fn write_saved_optional<L: NameLookup>(
    stack: Option<&ItemStack>,
    names: &L,
) -> Result<NbtCompound, SavedError> {
    match stack {
        Some(stack) => write_saved(stack, names),
        None => Ok(NbtCompound::new()),
    }
}

/// Reads the saved stack with slot. An absent slot is the slot 0.
pub fn read_saved_with_slot<L: NameLookup>(
    saved: &NbtCompound,
    names: &L,
) -> Result<SlotStack, SavedError> {
    let slot = match saved.get(SLOT) {
        None => 0,
        Some(NbtTag::Byte(slot)) => slot.cast_unsigned(),
        Some(_) => return Err(SavedError::BadSlot),
    };
    let stack = read_saved(saved, names)?;
    Ok(SlotStack { slot, stack })
}

/// Writes the saved stack with slot.
pub fn write_saved_with_slot<L: NameLookup>(
    slotted: &SlotStack,
    names: &L,
) -> Result<NbtCompound, SavedError> {
    let mut saved = write_saved(&slotted.stack, names)?;
    saved.put(SLOT, NbtTag::Byte(slotted.slot.cast_signed()));
    Ok(saved)
}

pub(crate) fn is_air(item: ResourceLocation<'_>) -> bool {
    item.namespace() == NAMESPACE && item.path() == AIR
}

fn read_entry<L: NameLookup>(
    key: SavedKey<'_>,
    value: &NbtTag,
    names: &L,
) -> Result<PatchEntry, SavedError> {
    let name = key.name;
    let builtin = BuiltinKey::from_location(name);
    if builtin.is_none() && names.id(NameTable::ComponentType, name).is_none() {
        return Err(SavedError::UnknownComponentType);
    }
    if key.kind == KeyKind::Removal {
        if !matches!(value, NbtTag::Compound(_)) {
            return Err(SavedError::BadRemovalValue);
        }
        return Ok(match builtin {
            Some(component) => PatchEntry::Removal(component),
            None => PatchEntry::UndecodedRemoval(name.to_buf()),
        });
    }
    match builtin {
        Some(component) => Ok(PatchEntry::Value(read_value(component, value, names)?)),
        None => Ok(PatchEntry::Undecoded(UndecodedValue {
            name: name.to_buf(),
            bytes: encode(value)?,
        })),
    }
}

fn write_entry<L: NameLookup>(
    entry: &PatchEntry,
    names: &L,
    patch: &mut NbtCompound,
) -> Result<(), SavedError> {
    match entry {
        PatchEntry::Value(value) => {
            let key = format!("{NAMESPACE}:{}", value.key().path());
            patch.put(&key, write_value(value, names)?);
        }
        PatchEntry::Removal(component) => {
            let key = format!("{REMOVAL_MARKER}{NAMESPACE}:{}", component.path());
            patch.put(&key, NbtTag::Compound(NbtCompound::new()));
        }
        PatchEntry::Undecoded(value) => patch.put(value.name.as_str(), decode(&value.bytes)?),
        PatchEntry::UndecodedRemoval(name) => {
            let key = format!("{REMOVAL_MARKER}{name}");
            patch.put(&key, NbtTag::Compound(NbtCompound::new()));
        }
        PatchEntry::UndecodedWire(value) => {
            return Err(SavedError::WireValueOnly(value.component()));
        }
        PatchEntry::Custom(_) => {}
    }
    Ok(())
}

fn encode(tag: &NbtTag) -> Result<Box<[u8]>, SavedError> {
    let mut writer = WriteAdaptor::new(Vec::new());
    tag.serialize(&mut writer)
        .map_err(|_| SavedError::BadUndecodedBytes)?;
    Ok(writer.into_inner().into_boxed_slice())
}

fn decode(bytes: &[u8]) -> Result<NbtTag, SavedError> {
    let mut reader = NbtReadHelper::new(Cursor::new(bytes));
    let tag = NbtTag::deserialize(&mut reader).map_err(|_| SavedError::BadUndecodedBytes)?;
    let end = reader
        .position()
        .map_err(|_| SavedError::BadUndecodedBytes)?;
    let is_whole = usize::try_from(end).is_ok_and(|end| end == bytes.len());
    if matches!(tag, NbtTag::End) || !is_whole {
        return Err(SavedError::BadUndecodedBytes);
    }
    Ok(tag)
}

pub(crate) fn read_value<L: NameLookup>(
    component: BuiltinKey,
    tag: &NbtTag,
    names: &L,
) -> Result<Builtin, SavedError> {
    let reader = Reader { component, names };
    Ok(match component {
        BuiltinKey::MaxStackSize => {
            Builtin::MaxStackSize(MaxStackSize(reader.int(tag, STACK_SIZE)?))
        }
        BuiltinKey::MaxDamage => Builtin::MaxDamage(MaxDamage(reader.int(tag, POSITIVE)?)),
        BuiltinKey::Damage => Builtin::Damage(Damage(reader.int(tag, NOT_NEGATIVE)?)),
        BuiltinKey::RepairCost => Builtin::RepairCost(RepairCost(reader.int(tag, NOT_NEGATIVE)?)),
        BuiltinKey::Unbreakable => {
            reader.map(tag)?;
            Builtin::Unbreakable(Unbreakable)
        }
        BuiltinKey::ItemModel => {
            Builtin::ItemModel(ItemModel::new(reader.identifier(tag)?.to_buf()))
        }
        BuiltinKey::Rarity => {
            let rarity = Rarity::from_name(reader.string(tag)?).ok_or(reader.bad())?;
            Builtin::Rarity(rarity)
        }
        BuiltinKey::Enchantments => Builtin::Enchantments(reader.enchantments(tag)?),
        BuiltinKey::AttributeModifiers => {
            Builtin::AttributeModifiers(reader.attribute_modifiers(tag)?)
        }
        BuiltinKey::TooltipDisplay => Builtin::TooltipDisplay(reader.tooltip_display(tag)?),
        BuiltinKey::Food => Builtin::Food(reader.food(tag)?),
        BuiltinKey::Tool => Builtin::Tool(reader.tool(tag)?),
        BuiltinKey::CustomName => Builtin::CustomName(CustomName(read_text(tag, component)?)),
        BuiltinKey::ItemName => Builtin::ItemName(ItemName(read_text(tag, component)?)),
        BuiltinKey::Lore => Builtin::Lore(read_lore(tag)?),
    })
}

struct Reader<'names, L> {
    component: BuiltinKey,
    names: &'names L,
}

impl<L: NameLookup> Reader<'_, L> {
    fn bad(&self) -> SavedError {
        SavedError::BadValue(self.component)
    }

    fn number(&self, tag: &NbtTag) -> Result<TagNumber, SavedError> {
        TagNumber::of(tag).ok_or(self.bad())
    }

    fn int(&self, tag: &NbtTag, range: RangeInclusive<i32>) -> Result<i32, SavedError> {
        let value = self.number(tag)?.to_int();
        if range.contains(&value) {
            Ok(value)
        } else {
            Err(SavedError::OutOfRange(self.component))
        }
    }

    fn float(&self, tag: &NbtTag) -> Result<f32, SavedError> {
        Ok(self.number(tag)?.to_float())
    }

    fn boolean(&self, tag: &NbtTag) -> Result<bool, SavedError> {
        Ok(self.number(tag)?.is_true())
    }

    fn string<'tag>(&self, tag: &'tag NbtTag) -> Result<&'tag str, SavedError> {
        tag.extract_string().ok_or(self.bad())
    }

    fn map<'tag>(&self, tag: &'tag NbtTag) -> Result<&'tag NbtCompound, SavedError> {
        tag.extract_compound().ok_or(self.bad())
    }

    fn list<'tag>(&self, tag: &'tag NbtTag) -> Result<&'tag [NbtTag], SavedError> {
        tag.extract_list().ok_or(self.bad())
    }

    fn necessary<'tag>(
        &self,
        map: &'tag NbtCompound,
        field: &str,
    ) -> Result<&'tag NbtTag, SavedError> {
        map.get(field).ok_or(self.bad())
    }

    fn location<'text>(&self, text: &'text str) -> Result<ResourceLocation<'text>, SavedError> {
        ResourceLocation::read(text).map_err(|_| self.bad())
    }

    fn identifier<'tag>(&self, tag: &'tag NbtTag) -> Result<ResourceLocation<'tag>, SavedError> {
        self.location(self.string(tag)?)
    }

    fn entry(&self, table: NameTable, name: ResourceLocation<'_>) -> Result<u16, SavedError> {
        self.names.id(table, name).ok_or(SavedError::UnknownEntry {
            component: self.component,
            table,
        })
    }

    fn enchantments(&self, tag: &NbtTag) -> Result<Enchantments, SavedError> {
        let map = self.map(tag)?;
        let mut entries: Vec<EnchantmentLevel> = Vec::with_capacity(map.child_tags.len());
        for (name, level) in &map.child_tags {
            let enchantment = self.entry(NameTable::Enchantment, self.location(name)?)?;
            let level = u8::try_from(self.int(level, 1..=255)?)
                .ok()
                .and_then(NonZeroU8::new)
                .ok_or(SavedError::OutOfRange(self.component))?;
            entries.push(EnchantmentLevel {
                enchantment: EnchantmentId(enchantment),
                level,
            });
        }
        Enchantments::new(entries).ok_or(self.bad())
    }

    fn attribute_modifiers(&self, tag: &NbtTag) -> Result<AttributeModifiers, SavedError> {
        let list = self.list(tag)?;
        let mut modifiers: Vec<AttributeModifier> = Vec::with_capacity(list.len());
        for entry in list {
            let entry = self.map(entry)?;
            let attribute = self.identifier(self.necessary(entry, "type")?)?;
            let attribute = AttributeId(self.entry(NameTable::Attribute, attribute)?);
            let id = self.identifier(self.necessary(entry, "id")?)?.to_buf();
            let amount = self.number(self.necessary(entry, "amount")?)?.to_double();
            let operation = self.string(self.necessary(entry, "operation")?)?;
            let operation = ModifierOperation::from_name(operation).ok_or(self.bad())?;
            let mut modifier = AttributeModifier::new(attribute, id, amount, operation);
            if let Some(slot) = entry.get("slot") {
                modifier.slot = SlotGroup::from_name(self.string(slot)?).ok_or(self.bad())?;
            }
            if let Some(display) = entry.get("display") {
                let display = self.map(display)?;
                match self.string(self.necessary(display, "type")?)? {
                    DISPLAY_DEFAULT => {}
                    DISPLAY_HIDDEN => modifier.display = ModifierDisplay::Hidden,
                    DISPLAY_OVERRIDE => {
                        let text = read_text(self.necessary(display, "value")?, self.component)?;
                        modifier.display = ModifierDisplay::Override(text);
                    }
                    _ => return Err(self.bad()),
                }
            }
            modifiers.push(modifier);
        }
        Ok(AttributeModifiers {
            modifiers: Cow::Owned(modifiers),
        })
    }

    fn tooltip_display(&self, tag: &NbtTag) -> Result<TooltipDisplay, SavedError> {
        let map = self.map(tag)?;
        let mut hidden: Vec<ComponentType> = Vec::new();
        if let Some(list) = map.get("hidden_components") {
            for element in self.list(list)? {
                let name = self.identifier(element)?;
                hidden.push(ComponentType(self.entry(NameTable::ComponentType, name)?));
            }
        }
        let mut display = TooltipDisplay::hiding(hidden);
        if let Some(hide_tooltip) = map.get("hide_tooltip") {
            display.hide_tooltip = self.boolean(hide_tooltip)?;
        }
        Ok(display)
    }

    fn food(&self, tag: &NbtTag) -> Result<Food, SavedError> {
        let map = self.map(tag)?;
        let mut food = Food {
            nutrition: self.int(self.necessary(map, "nutrition")?, NOT_NEGATIVE)?,
            saturation: self.float(self.necessary(map, "saturation")?)?,
            can_always_eat: false,
        };
        if let Some(can_always_eat) = map.get("can_always_eat") {
            food.can_always_eat = self.boolean(can_always_eat)?;
        }
        Ok(food)
    }

    fn tool(&self, tag: &NbtTag) -> Result<Tool, SavedError> {
        let map = self.map(tag)?;
        let list = self.list(self.necessary(map, "rules")?)?;
        let mut rules: Vec<ToolRule> = Vec::with_capacity(list.len());
        for rule in list {
            rules.push(self.tool_rule(rule)?);
        }
        let mut tool = Tool::new(rules);
        if let Some(speed) = map.get("default_mining_speed") {
            tool.default_mining_speed = self.float(speed)?;
        }
        if let Some(damage) = map.get("damage_per_block") {
            tool.damage_per_block = self.int(damage, NOT_NEGATIVE)?;
        }
        if let Some(can_destroy) = map.get("can_destroy_blocks_in_creative") {
            tool.can_destroy_blocks_in_creative = self.boolean(can_destroy)?;
        }
        Ok(tool)
    }

    fn tool_rule(&self, tag: &NbtTag) -> Result<ToolRule, SavedError> {
        let map = self.map(tag)?;
        let mut rule = ToolRule::new(self.block_set(self.necessary(map, "blocks")?)?);
        if let Some(speed) = map.get("speed") {
            rule = rule
                .with_speed(self.float(speed)?)
                .ok_or(SavedError::OutOfRange(self.component))?;
        }
        if let Some(correct_for_drops) = map.get("correct_for_drops") {
            rule.correct_for_drops = Some(self.boolean(correct_for_drops)?);
        }
        Ok(rule)
    }

    fn block_set(&self, tag: &NbtTag) -> Result<BlockSet, SavedError> {
        let mut blocks: Vec<BlockId> = Vec::new();
        match tag {
            NbtTag::String(text) => {
                if let Some(block_tag) = text.strip_prefix(TAG_MARKER) {
                    let block_tag = self.entry(NameTable::BlockTag, self.location(block_tag)?)?;
                    return Ok(BlockSet::Tag(BlockTagId(block_tag)));
                }
                blocks.push(self.block(text)?);
            }
            NbtTag::List(list) => {
                for element in list {
                    blocks.push(self.block(self.string(element)?)?);
                }
            }
            _ => return Err(self.bad()),
        }
        Ok(BlockSet::Blocks(Cow::Owned(blocks)))
    }

    fn block(&self, text: &str) -> Result<BlockId, SavedError> {
        self.entry(NameTable::Block, self.location(text)?)
            .map(BlockId)
    }
}

fn write_value<L: NameLookup>(value: &Builtin, names: &L) -> Result<NbtTag, SavedError> {
    let writer = Writer {
        component: value.key(),
        names,
    };
    match value {
        Builtin::MaxStackSize(MaxStackSize(size)) => writer.int(*size, STACK_SIZE),
        Builtin::MaxDamage(MaxDamage(damage)) => writer.int(*damage, POSITIVE),
        Builtin::Damage(Damage(damage)) => writer.int(*damage, NOT_NEGATIVE),
        Builtin::RepairCost(RepairCost(cost)) => writer.int(*cost, NOT_NEGATIVE),
        Builtin::Unbreakable(Unbreakable) => Ok(NbtTag::Compound(NbtCompound::new())),
        Builtin::ItemModel(model) => Ok(NbtTag::String(model.get().to_string())),
        Builtin::Rarity(rarity) => Ok(name_tag(rarity.name())),
        Builtin::Enchantments(enchantments) => writer.enchantments(enchantments),
        Builtin::AttributeModifiers(modifiers) => writer.attribute_modifiers(modifiers),
        Builtin::TooltipDisplay(display) => writer.tooltip_display(display),
        Builtin::Food(food) => writer.food(food),
        Builtin::Tool(tool) => writer.tool(tool),
        Builtin::CustomName(CustomName(text)) | Builtin::ItemName(ItemName(text)) => {
            write_text(text, writer.component)
        }
        Builtin::Lore(lore) => write_lore(lore),
    }
}

fn name_tag(name: &str) -> NbtTag {
    NbtTag::String(name.into())
}

fn boolean_tag(value: bool) -> NbtTag {
    NbtTag::Byte(i8::from(value))
}

struct Writer<'names, L> {
    component: BuiltinKey,
    names: &'names L,
}

impl<L: NameLookup> Writer<'_, L> {
    fn int(&self, value: i32, range: RangeInclusive<i32>) -> Result<NbtTag, SavedError> {
        if range.contains(&value) {
            Ok(NbtTag::Int(value))
        } else {
            Err(SavedError::OutOfRange(self.component))
        }
    }

    fn name(&self, table: NameTable, id: u16) -> Result<ResourceLocation<'_>, SavedError> {
        self.names
            .name(table, id)
            .ok_or(SavedError::UnnamedId(table))
    }

    fn entry(&self, table: NameTable, id: u16) -> Result<NbtTag, SavedError> {
        Ok(NbtTag::String(self.name(table, id)?.to_string()))
    }

    fn enchantments(&self, enchantments: &Enchantments) -> Result<NbtTag, SavedError> {
        let mut map = NbtCompound::new();
        for entry in enchantments.entries() {
            let name = self.name(NameTable::Enchantment, entry.enchantment.0)?;
            map.put(&name.to_string(), NbtTag::Int(i32::from(entry.level.get())));
        }
        Ok(NbtTag::Compound(map))
    }

    fn attribute_modifiers(&self, modifiers: &AttributeModifiers) -> Result<NbtTag, SavedError> {
        let mut list: Vec<NbtTag> = Vec::with_capacity(modifiers.modifiers.len());
        for modifier in modifiers.modifiers.iter() {
            let mut entry = NbtCompound::new();
            entry.put(
                "type",
                self.entry(NameTable::Attribute, modifier.attribute.0)?,
            );
            entry.put("id", NbtTag::String(modifier.id().to_string()));
            entry.put("amount", NbtTag::Double(modifier.amount));
            entry.put("operation", name_tag(modifier.operation.name()));
            if modifier.slot != SlotGroup::Any {
                entry.put("slot", name_tag(modifier.slot.name()));
            }
            match &modifier.display {
                ModifierDisplay::Default => {}
                ModifierDisplay::Hidden => {
                    let mut display = NbtCompound::new();
                    display.put("type", name_tag(DISPLAY_HIDDEN));
                    entry.put("display", NbtTag::Compound(display));
                }
                ModifierDisplay::Override(text) => {
                    let mut display = NbtCompound::new();
                    display.put("type", name_tag(DISPLAY_OVERRIDE));
                    display.put("value", write_text(text, self.component)?);
                    entry.put("display", NbtTag::Compound(display));
                }
            }
            list.push(NbtTag::Compound(entry));
        }
        Ok(NbtTag::List(list))
    }

    fn tooltip_display(&self, display: &TooltipDisplay) -> Result<NbtTag, SavedError> {
        let mut map = NbtCompound::new();
        if display.hide_tooltip {
            map.put("hide_tooltip", boolean_tag(true));
        }
        let hidden = display.hidden_components();
        if !hidden.is_empty() {
            let mut list: Vec<NbtTag> = Vec::with_capacity(hidden.len());
            for component_type in hidden {
                list.push(self.entry(NameTable::ComponentType, component_type.0)?);
            }
            map.put("hidden_components", NbtTag::List(list));
        }
        Ok(NbtTag::Compound(map))
    }

    fn food(&self, food: &Food) -> Result<NbtTag, SavedError> {
        let mut map = NbtCompound::new();
        map.put("nutrition", self.int(food.nutrition, NOT_NEGATIVE)?);
        map.put("saturation", NbtTag::Float(food.saturation));
        if food.can_always_eat {
            map.put("can_always_eat", boolean_tag(true));
        }
        Ok(NbtTag::Compound(map))
    }

    fn tool(&self, tool: &Tool) -> Result<NbtTag, SavedError> {
        let mut rules: Vec<NbtTag> = Vec::with_capacity(tool.rules.len());
        for rule in tool.rules.iter() {
            rules.push(self.tool_rule(rule)?);
        }
        let mut map = NbtCompound::new();
        map.put("rules", NbtTag::List(rules));
        if tool.default_mining_speed.to_bits() != DEFAULT_MINING_SPEED.to_bits() {
            map.put(
                "default_mining_speed",
                NbtTag::Float(tool.default_mining_speed),
            );
        }
        if tool.damage_per_block != DEFAULT_DAMAGE_PER_BLOCK {
            map.put(
                "damage_per_block",
                self.int(tool.damage_per_block, NOT_NEGATIVE)?,
            );
        }
        if !tool.can_destroy_blocks_in_creative {
            map.put("can_destroy_blocks_in_creative", boolean_tag(false));
        }
        Ok(NbtTag::Compound(map))
    }

    fn tool_rule(&self, rule: &ToolRule) -> Result<NbtTag, SavedError> {
        let mut map = NbtCompound::new();
        map.put("blocks", self.block_set(&rule.blocks)?);
        if let Some(speed) = rule.speed() {
            map.put("speed", NbtTag::Float(speed));
        }
        if let Some(correct_for_drops) = rule.correct_for_drops {
            map.put("correct_for_drops", boolean_tag(correct_for_drops));
        }
        Ok(NbtTag::Compound(map))
    }

    fn block_set(&self, blocks: &BlockSet) -> Result<NbtTag, SavedError> {
        match blocks {
            BlockSet::Tag(block_tag) => {
                let name = self.name(NameTable::BlockTag, block_tag.0)?;
                Ok(NbtTag::String(format!("{TAG_MARKER}{name}")))
            }
            BlockSet::Blocks(blocks) => {
                if let [block] = blocks.as_ref() {
                    return self.entry(NameTable::Block, block.0);
                }
                let mut list: Vec<NbtTag> = Vec::with_capacity(blocks.len());
                for block in blocks.iter() {
                    list.push(self.entry(NameTable::Block, block.0)?);
                }
                Ok(NbtTag::List(list))
            }
        }
    }
}
