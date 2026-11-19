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
use mcrs_minecraft_nbt::from_tag;
use mcrs_minecraft_nbt::serializer::WriteAdaptor;
use mcrs_minecraft_nbt::tag::NbtTag;
use mcrs_minecraft_resource::ResourceLocation;
use mcrs_minecraft_snbt::parse_compound;
use mcrs_voxel::BlockPos;

use crate::ArgbColor;
use crate::AttackAnimation;
use crate::AttributeId;
use crate::AttributeModifier;
use crate::AttributeModifiers;
use crate::BannerLayer;
use crate::BannerPatternId;
use crate::BannerPatterns;
use crate::BeeOccupant;
use crate::Bees;
use crate::BlockId;
use crate::BlockSet;
use crate::BlockState;
use crate::BlockStateProperty;
use crate::BlockTagId;
use crate::Builtin;
use crate::BuiltinKey;
use crate::BundleContents;
use crate::ChargedProjectiles;
use crate::ComponentType;
use crate::Container;
use crate::ContainerLoot;
use crate::Count;
use crate::CustomData;
use crate::CustomModelData;
use crate::CustomName;
use crate::Damage;
use crate::DimensionPos;
use crate::DyeColor;
use crate::DyedColor;
use crate::EffectInstance;
use crate::EnchantmentGlintOverride;
use crate::EnchantmentId;
use crate::EnchantmentLevel;
use crate::Enchantments;
use crate::EntityData;
use crate::EntityTypeId;
use crate::EntityTypeSet;
use crate::EntityTypeTagId;
use crate::Equippable;
use crate::EquippableSlot;
use crate::FireworkExplosion;
use crate::FireworkShape;
use crate::Fireworks;
use crate::Food;
use crate::InlineBannerPattern;
use crate::InlineInstrument;
use crate::InlineSoundEvent;
use crate::InlineTrimMaterial;
use crate::InlineTrimPattern;
use crate::Instrument;
use crate::InstrumentId;
use crate::InteractAnimation;
use crate::ItemModel;
use crate::ItemName;
use crate::ItemStack;
use crate::Kind;
use crate::LodestoneTracker;
use crate::MapDecoration;
use crate::MapDecorationTypeId;
use crate::MapDecorations;
use crate::MapId;
use crate::MaxDamage;
use crate::MaxStackSize;
use crate::MobEffectId;
use crate::ModifierDisplay;
use crate::NameLookup;
use crate::NameTable;
use crate::OminousBottleAmplifier;
use crate::PaintingVariant;
use crate::PaintingVariantId;
use crate::PatchEntry;
use crate::PiercingWeapon;
use crate::PotionContents;
use crate::PotionId;
use crate::Profile;
use crate::Rarity;
use crate::RegistryValue;
use crate::RepairCost;
use crate::SavedError;
use crate::SlotGroup;
use crate::SlotStack;
use crate::SoundEventId;
use crate::SoundEventRef;
use crate::StewEffect;
use crate::StoredEnchantments;
use crate::SuspiciousStewEffects;
use crate::SwingAnimation;
use crate::SwingKind;
use crate::Tool;
use crate::ToolRule;
use crate::TooltipDisplay;
use crate::Trim;
use crate::TrimMaterialId;
use crate::TrimPatternId;
use crate::Unbreakable;
use crate::UndecodedValue;
use crate::builtin_key::NAMESPACE;
use crate::check_saved_form::MAX_COUNT;
use crate::check_saved_form::NOT_NEGATIVE;
use crate::check_saved_form::OMINOUS_AMPLIFIER;
use crate::check_saved_form::POSITIVE;
use crate::check_saved_form::STACK_SIZE;
use crate::entity_data::ENTITY_TYPE_KEY;
use crate::nbt_depth::LIST_TEMPLATE_LEVELS;
use crate::nbt_depth::SLOT_TEMPLATE_LEVELS;
use crate::nbt_depth::STACK_LEVEL;
use crate::nbt_depth::check_nbt_depth;
use crate::saved_form::DISPLAY_DEFAULT;
use crate::saved_form::DISPLAY_HIDDEN;
use crate::saved_form::DISPLAY_OVERRIDE;
use crate::saved_form::Fault;
use crate::saved_form::is_instrument_range;
use crate::saved_form::is_use_duration;
use crate::saved_form::saved_form;
use crate::saved_key::KeyKind;
use crate::saved_key::REMOVAL_MARKER;
use crate::saved_key::SavedKey;
use crate::saved_sink::TAG_MARKER;
use crate::saved_text::read_lore;
use crate::saved_text::read_text;
use crate::tag_number::TagNumber;
use crate::tag_sink::TagSink;
use mcrs_minecraft_entity_attribute::ModifierOperation;
use mcrs_minecraft_nbt::MAX_DEPTH;

const ID: &str = "id";
const COUNT: &str = "count";
const COMPONENTS: &str = "components";
const SLOT: &str = "Slot";
const AIR: &str = "air";

pub fn read_saved<L: NameLookup>(saved: &NbtCompound, names: &L) -> Result<ItemStack, SavedError> {
    let stack = read_stack(saved, names, STACK_LEVEL)?;
    check_nbt_depth(&stack, names).map_err(Fault::into_saved)?;
    Ok(stack)
}

fn read_item<L: NameLookup>(id: &str, names: &L) -> Result<Kind, SavedError> {
    let item = ResourceLocation::read(id).map_err(|_| SavedError::BadItemId)?;
    if is_air(item) {
        return Err(SavedError::AirItem);
    }
    names
        .id(NameTable::Item, item)
        .map(Kind)
        .ok_or(SavedError::UnknownItem)
}

fn read_stack<L: NameLookup>(
    saved: &NbtCompound,
    names: &L,
    depth: u32,
) -> Result<ItemStack, SavedError> {
    let Some(NbtTag::String(id)) = saved.get(ID) else {
        return Err(SavedError::BadItemId);
    };
    let kind = read_item(id, names)?;
    let count = match saved.get(COUNT) {
        None => 1,
        Some(count) => TagNumber::of(count).ok_or(SavedError::BadCount)?.to_int(),
    };
    let count = u8::try_from(count)
        .ok()
        .filter(|count| *count <= MAX_COUNT)
        .and_then(Count::new)
        .ok_or(SavedError::BadCount)?;
    let mut stack = ItemStack::new(kind, count);
    match saved.get(COMPONENTS) {
        None => {}
        Some(NbtTag::Compound(patch)) => {
            for (index, (key, value)) in patch.child_tags.iter().enumerate() {
                let key = SavedKey::read(key)?;
                if key.is_repeated(patch.child_tags.get(..index).unwrap_or_default()) {
                    return Err(SavedError::RepeatedKey);
                }
                read_entry(key, value, names, depth).map(|entry| stack.put_entry(entry))?;
            }
        }
        Some(_) => return Err(SavedError::BadPatch),
    }
    Ok(stack)
}

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
    check_nbt_depth(stack, names).map_err(Fault::into_saved)?;
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

pub fn write_saved_optional<L: NameLookup>(
    stack: Option<&ItemStack>,
    names: &L,
) -> Result<NbtCompound, SavedError> {
    match stack {
        Some(stack) => write_saved(stack, names),
        None => Ok(NbtCompound::new()),
    }
}

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
    depth: u32,
) -> Result<PatchEntry, SavedError> {
    let name = key.name;
    let builtin = BuiltinKey::from_location(name);
    if builtin.is_none() && names.id(NameTable::ComponentType, name).is_none() {
        return Err(SavedError::UnknownComponentType);
    }
    match (key.kind, builtin) {
        (KeyKind::Removal, _) => read_removal(builtin, name, value),
        (KeyKind::Value, Some(component)) => {
            read_value_at(component, value, names, depth).map(PatchEntry::Value)
        }
        (KeyKind::Value, None) => read_undecoded(name, value),
    }
}

#[inline(never)]
fn read_removal(
    builtin: Option<BuiltinKey>,
    name: ResourceLocation<'_>,
    value: &NbtTag,
) -> Result<PatchEntry, SavedError> {
    if !matches!(value, NbtTag::Compound(_)) {
        return Err(SavedError::BadRemovalValue);
    }
    Ok(match builtin {
        Some(component) => PatchEntry::Removal(component),
        None => PatchEntry::UndecodedRemoval(name.to_buf()),
    })
}

#[inline(never)]
fn read_undecoded(name: ResourceLocation<'_>, value: &NbtTag) -> Result<PatchEntry, SavedError> {
    Ok(PatchEntry::Undecoded(UndecodedValue {
        name: name.to_buf(),
        bytes: encode(value)?,
    }))
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

pub(crate) fn decode(bytes: &[u8]) -> Result<NbtTag, SavedError> {
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
    read_value_at(component, tag, names, STACK_LEVEL)
}

fn read_value_at<L: NameLookup>(
    component: BuiltinKey,
    tag: &NbtTag,
    names: &L,
    depth: u32,
) -> Result<Builtin, SavedError> {
    let reader = Reader {
        component,
        names,
        depth,
    };
    match component {
        BuiltinKey::ChargedProjectiles => {
            let list = reader.list(tag)?;
            if list.len() > ChargedProjectiles::MAX_PROJECTILES {
                return Err(SavedError::OutOfRange(component));
            }
            reader
                .templates(list)
                .map(|stacks| Builtin::ChargedProjectiles(ChargedProjectiles(stacks)))
        }
        BuiltinKey::BundleContents => reader
            .templates(reader.list(tag)?)
            .map(|stacks| Builtin::BundleContents(BundleContents(stacks))),
        BuiltinKey::Container => reader.container(tag).map(Builtin::Container),
        _ => read_flat_value(&reader, tag),
    }
}

#[inline(never)]
fn read_flat_value<L: NameLookup>(
    reader: &Reader<'_, L>,
    tag: &NbtTag,
) -> Result<Builtin, SavedError> {
    let component = reader.component;
    Ok(match component {
        BuiltinKey::MaxStackSize => {
            Builtin::MaxStackSize(MaxStackSize(reader.int(tag, STACK_SIZE)?))
        }
        BuiltinKey::MaxDamage => Builtin::MaxDamage(MaxDamage(reader.int(tag, POSITIVE)?)),
        BuiltinKey::Damage => Builtin::Damage(Damage(reader.int(tag, NOT_NEGATIVE)?)),
        BuiltinKey::RepairCost => Builtin::RepairCost(RepairCost(reader.int(tag, NOT_NEGATIVE)?)),
        BuiltinKey::MapId => Builtin::MapId(MapId(reader.number(tag)?.to_int())),
        BuiltinKey::MapDecorations => Builtin::MapDecorations(reader.map_decorations(tag)?),
        BuiltinKey::Profile => Builtin::Profile(reader.profile(tag)?),
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
        BuiltinKey::StoredEnchantments => {
            Builtin::StoredEnchantments(StoredEnchantments(reader.enchantments(tag)?))
        }
        BuiltinKey::PaintingVariant => Builtin::PaintingVariant(PaintingVariant(
            PaintingVariantId(reader.entry(NameTable::PaintingVariant, reader.identifier(tag)?)?),
        )),
        BuiltinKey::AttributeModifiers => {
            Builtin::AttributeModifiers(reader.attribute_modifiers(tag)?)
        }
        BuiltinKey::TooltipDisplay => Builtin::TooltipDisplay(reader.tooltip_display(tag)?),
        BuiltinKey::Food => Builtin::Food(reader.food(tag)?),
        BuiltinKey::Tool => Builtin::Tool(reader.tool(tag)?),
        BuiltinKey::Equippable => Builtin::Equippable(reader.equippable(tag)?),
        BuiltinKey::PiercingWeapon => Builtin::PiercingWeapon(reader.piercing_weapon(tag)?),
        BuiltinKey::AttackAnimation => {
            Builtin::AttackAnimation(AttackAnimation(reader.swing_animation(tag)?))
        }
        BuiltinKey::InteractAnimation => {
            Builtin::InteractAnimation(InteractAnimation(reader.swing_animation(tag)?))
        }
        BuiltinKey::CustomName => Builtin::CustomName(CustomName(read_text(tag, component)?)),
        BuiltinKey::ItemName => Builtin::ItemName(ItemName(read_text(tag, component)?)),
        BuiltinKey::Lore => Builtin::Lore(read_lore(tag)?),
        BuiltinKey::DyedColor => Builtin::DyedColor(DyedColor(reader.color(tag)?)),
        BuiltinKey::CustomModelData => Builtin::CustomModelData(reader.custom_model_data(tag)?),
        BuiltinKey::EnchantmentGlintOverride => {
            Builtin::EnchantmentGlintOverride(EnchantmentGlintOverride(reader.boolean(tag)?))
        }
        BuiltinKey::BlockState => Builtin::BlockState(reader.block_state(tag)?),
        BuiltinKey::LodestoneTracker => Builtin::LodestoneTracker(reader.lodestone_tracker(tag)?),
        BuiltinKey::Trim => Builtin::Trim(reader.trim(tag)?),
        BuiltinKey::PotionContents => Builtin::PotionContents(reader.potion_contents(tag)?),
        BuiltinKey::CustomData => Builtin::CustomData(CustomData(reader.custom_data(tag)?)),
        BuiltinKey::SuspiciousStewEffects => {
            Builtin::SuspiciousStewEffects(reader.suspicious_stew_effects(tag)?)
        }
        BuiltinKey::OminousBottleAmplifier => Builtin::OminousBottleAmplifier(
            OminousBottleAmplifier(reader.int(tag, OMINOUS_AMPLIFIER)?),
        ),
        BuiltinKey::Instrument => Builtin::Instrument(Instrument(reader.instrument(tag)?)),
        BuiltinKey::ChargedProjectiles | BuiltinKey::BundleContents | BuiltinKey::Container => {
            return read_value_at(component, tag, reader.names, reader.depth);
        }
        BuiltinKey::FireworkExplosion => {
            Builtin::FireworkExplosion(reader.firework_explosion(tag)?)
        }
        BuiltinKey::Fireworks => Builtin::Fireworks(reader.fireworks(tag)?),
        BuiltinKey::BannerPatterns => Builtin::BannerPatterns(BannerPatterns(
            reader.each(reader.list(tag)?, Reader::banner_layer)?,
        )),
        BuiltinKey::Bees => Builtin::Bees(Bees(reader.each(reader.list(tag)?, Reader::bee)?)),
        BuiltinKey::ContainerLoot => {
            let map = reader.map(tag)?;
            Builtin::ContainerLoot(ContainerLoot {
                loot_table: reader
                    .identifier(reader.necessary(map, "loot_table")?)?
                    .to_buf(),
                seed: (reader
                    .optional(map, "seed", |reader, tag| Ok(reader.number(tag)?.to_long()))?)
                .unwrap_or_default(),
            })
        }
    })
}

struct Reader<'names, L> {
    component: BuiltinKey,
    names: &'names L,
    depth: u32,
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
            let operation: ModifierOperation = operation.parse().map_err(|_| self.bad())?;
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

    fn map_decorations(&self, tag: &NbtTag) -> Result<MapDecorations, SavedError> {
        let map = self.map(tag)?;
        let mut decorations: Vec<MapDecoration> = Vec::with_capacity(map.child_tags.len());
        for (key, decoration) in &map.child_tags {
            let decoration = self.map(decoration)?;
            let kind = self.identifier(self.necessary(decoration, "type")?)?;
            decorations.push(MapDecoration {
                key: key.as_str().into(),
                kind: MapDecorationTypeId(self.entry(NameTable::MapDecorationType, kind)?),
                x: self.number(self.necessary(decoration, "x")?)?.to_double(),
                z: self.number(self.necessary(decoration, "z")?)?.to_double(),
                rotation: self.float(self.necessary(decoration, "rotation")?)?,
            });
        }
        MapDecorations::new(decorations).ok_or(self.bad())
    }

    fn profile(&self, tag: &NbtTag) -> Result<Profile, SavedError> {
        from_tag(tag.clone()).map_err(|_| self.bad())
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

    fn equippable(&self, tag: &NbtTag) -> Result<Equippable, SavedError> {
        let map = self.map(tag)?;
        let slot = self.string(self.necessary(map, "slot")?)?;
        let mut equippable = Equippable::new(EquippableSlot::from_name(slot).ok_or(self.bad())?);
        equippable.equip_sound = self.optional(map, "equip_sound", Self::sound_event)?;
        equippable.set_asset_id(self.optional(map, "asset_id", |reader, tag| {
            Ok(reader.identifier(tag)?.to_buf())
        })?);
        equippable.set_camera_overlay(self.optional(map, "camera_overlay", |reader, tag| {
            Ok(reader.identifier(tag)?.to_buf())
        })?);
        equippable.allowed_entities =
            self.optional(map, "allowed_entities", Self::entity_type_set)?;
        equippable.dispensable = self.flag(map, "dispensable", equippable.dispensable)?;
        equippable.swappable = self.flag(map, "swappable", equippable.swappable)?;
        equippable.damage_on_hurt = self.flag(map, "damage_on_hurt", equippable.damage_on_hurt)?;
        equippable.equip_on_interact =
            self.flag(map, "equip_on_interact", equippable.equip_on_interact)?;
        equippable.can_be_sheared = self.flag(map, "can_be_sheared", equippable.can_be_sheared)?;
        equippable.shearing_sound = self.optional(map, "shearing_sound", Self::sound_event)?;
        Ok(equippable)
    }

    fn optional<T>(
        &self,
        map: &NbtCompound,
        field: &str,
        read: impl FnOnce(&Self, &NbtTag) -> Result<T, SavedError>,
    ) -> Result<Option<T>, SavedError> {
        map.get(field).map(|tag| read(self, tag)).transpose()
    }

    fn flag(&self, map: &NbtCompound, field: &str, absent: bool) -> Result<bool, SavedError> {
        map.get(field).map_or(Ok(absent), |tag| self.boolean(tag))
    }

    fn sound_event(&self, tag: &NbtTag) -> Result<SoundEventRef, SavedError> {
        match tag {
            NbtTag::String(name) => {
                let id = self.entry(NameTable::SoundEvent, self.location(name)?)?;
                Ok(SoundEventRef::Reference(SoundEventId(id)))
            }
            NbtTag::Compound(map) => {
                let sound_id = self.identifier(self.necessary(map, "sound_id")?)?;
                let mut inline = InlineSoundEvent::new(sound_id.to_buf());
                inline.range = map
                    .get("range")
                    .and_then(TagNumber::of)
                    .map(TagNumber::to_float);
                Ok(SoundEventRef::Inline(inline))
            }
            _ => Err(self.bad()),
        }
    }

    fn entity_type_set(&self, tag: &NbtTag) -> Result<EntityTypeSet, SavedError> {
        let mut entity_types: Vec<EntityTypeId> = Vec::new();
        match tag {
            NbtTag::String(text) => {
                if let Some(entity_tag) = text.strip_prefix(TAG_MARKER) {
                    let entity_tag =
                        self.entry(NameTable::EntityTypeTag, self.location(entity_tag)?)?;
                    return Ok(EntityTypeSet::Tag(EntityTypeTagId(entity_tag)));
                }
                entity_types.push(self.entity_type(text)?);
            }
            NbtTag::List(list) => {
                for element in list {
                    entity_types.push(self.entity_type(self.string(element)?)?);
                }
            }
            _ => return Err(self.bad()),
        }
        Ok(EntityTypeSet::EntityTypes(Cow::Owned(entity_types)))
    }

    fn entity_type(&self, text: &str) -> Result<EntityTypeId, SavedError> {
        self.entry(NameTable::EntityType, self.location(text)?)
            .map(EntityTypeId)
    }

    fn color(&self, tag: &NbtTag) -> Result<ArgbColor, SavedError> {
        if let Some(number) = TagNumber::of(tag) {
            return Ok(ArgbColor(number.to_int()));
        }
        let [red, green, blue] = self.list(tag)? else {
            return Err(self.bad());
        };
        Ok(ArgbColor::from_rgb_floats(
            self.float(red)?,
            self.float(green)?,
            self.float(blue)?,
        ))
    }

    fn custom_model_data(&self, tag: &NbtTag) -> Result<CustomModelData, SavedError> {
        let map = self.map(tag)?;
        Ok(CustomModelData {
            floats: self.listed(map, "floats", Self::float)?,
            flags: self.listed(map, "flags", Self::boolean)?,
            strings: self.listed(map, "strings", |reader, tag| {
                Ok(reader.string(tag)?.to_string())
            })?,
            colors: self.listed(map, "colors", Self::color)?,
        })
    }

    fn listed<T>(
        &self,
        map: &NbtCompound,
        field: &str,
        read: impl FnMut(&Self, &NbtTag) -> Result<T, SavedError>,
    ) -> Result<Vec<T>, SavedError> {
        let Some(tag) = map.get(field) else {
            return Ok(Vec::new());
        };
        self.each(self.list(tag)?, read)
    }

    fn block_state(&self, tag: &NbtTag) -> Result<BlockState, SavedError> {
        let map = self.map(tag)?;
        let mut properties: Vec<BlockStateProperty> = Vec::with_capacity(map.child_tags.len());
        for (name, value) in &map.child_tags {
            properties.push(BlockStateProperty {
                name: name.clone(),
                value: self.string(value)?.to_string(),
            });
        }
        Ok(BlockState::new(properties))
    }

    fn lodestone_tracker(&self, tag: &NbtTag) -> Result<LodestoneTracker, SavedError> {
        let map = self.map(tag)?;
        Ok(LodestoneTracker {
            target: self.optional(map, "target", Self::global_pos)?,
            tracked: self.flag(map, "tracked", true)?,
        })
    }

    fn global_pos(&self, tag: &NbtTag) -> Result<DimensionPos, SavedError> {
        let map = self.map(tag)?;
        let dimension = self.identifier(self.necessary(map, "dimension")?)?.to_buf();
        let [x, y, z] = self.position(self.necessary(map, "pos")?)?;
        Ok(DimensionPos::new(dimension, BlockPos::new(x, y, z)))
    }

    fn position(&self, tag: &NbtTag) -> Result<[i32; 3], SavedError> {
        match tag {
            NbtTag::IntArray(values) => match values.as_slice() {
                [x, y, z] => Ok([*x, *y, *z]),
                _ => Err(self.bad()),
            },
            NbtTag::ByteArray(values) => match values.as_ref() {
                [x, y, z] => Ok([x, y, z].map(|value| i32::from(value.cast_signed()))),
                _ => Err(self.bad()),
            },
            NbtTag::LongArray(values) => match values.as_slice() {
                [x, y, z] => Ok([x, y, z].map(|value| TagNumber::Integer(*value).to_int())),
                _ => Err(self.bad()),
            },
            NbtTag::List(values) => match values.as_slice() {
                [x, y, z] => Ok([
                    self.number(x)?.to_int(),
                    self.number(y)?.to_int(),
                    self.number(z)?.to_int(),
                ]),
                _ => Err(self.bad()),
            },
            _ => Err(self.bad()),
        }
    }

    fn trim(&self, tag: &NbtTag) -> Result<Trim, SavedError> {
        let map = self.map(tag)?;
        let material = match self.necessary(map, "material")? {
            NbtTag::Compound(material) => RegistryValue::Inline(InlineTrimMaterial::new(
                self.identifier(self.necessary(material, "palette_id")?)?
                    .to_buf(),
                read_text(self.necessary(material, "description")?, self.component)?,
            )),
            material => RegistryValue::Reference(TrimMaterialId(
                self.entry(NameTable::TrimMaterial, self.identifier(material)?)?,
            )),
        };
        let pattern = match self.necessary(map, "pattern")? {
            NbtTag::Compound(pattern) => {
                let mut inline = InlineTrimPattern::new(
                    self.identifier(self.necessary(pattern, "asset_id")?)?
                        .to_buf(),
                    read_text(self.necessary(pattern, "description")?, self.component)?,
                );
                inline.decal = self.flag(pattern, "decal", false)?;
                RegistryValue::Inline(inline)
            }
            pattern => RegistryValue::Reference(TrimPatternId(
                self.entry(NameTable::TrimPattern, self.identifier(pattern)?)?,
            )),
        };
        Ok(Trim { material, pattern })
    }

    fn potion(&self, tag: &NbtTag) -> Result<PotionId, SavedError> {
        self.entry(NameTable::Potion, self.identifier(tag)?)
            .map(PotionId)
    }

    fn mob_effect(&self, tag: &NbtTag) -> Result<MobEffectId, SavedError> {
        self.entry(NameTable::MobEffect, self.identifier(tag)?)
            .map(MobEffectId)
    }

    fn potion_contents(&self, tag: &NbtTag) -> Result<PotionContents, SavedError> {
        let map = match tag {
            NbtTag::String(_) => {
                return Ok(PotionContents {
                    potion: Some(self.potion(tag)?),
                    ..PotionContents::default()
                });
            }
            _ => self.map(tag)?,
        };
        Ok(PotionContents {
            potion: self.optional(map, "potion", Self::potion)?,
            custom_color: self.optional(map, "custom_color", |reader, tag| {
                Ok(reader.number(tag)?.to_int())
            })?,
            custom_effects: self.listed(map, "custom_effects", Self::effect_instance)?,
            custom_name: self.optional(map, "custom_name", |reader, tag| {
                Ok(reader.string(tag)?.to_string())
            })?,
        })
    }

    fn effect_instance(&self, tag: &NbtTag) -> Result<EffectInstance, SavedError> {
        let map = self.map(tag)?;
        let mut effect = EffectInstance::new(self.mob_effect(self.necessary(map, "id")?)?);
        if let Some(amplifier) = map.get("amplifier") {
            effect.amplifier = self.number(amplifier)?.to_int() as u8;
        }
        if let Some(duration) = map.get("duration") {
            effect.duration = self.number(duration)?.to_int();
        }
        effect.ambient = self.flag(map, "ambient", false)?;
        effect.show_particles = self.flag(map, "show_particles", true)?;
        effect.show_icon = self.flag(map, "show_icon", effect.show_particles)?;
        Ok(effect)
    }

    fn custom_data(&self, tag: &NbtTag) -> Result<NbtCompound, SavedError> {
        match tag {
            NbtTag::Compound(compound) => Ok(compound.clone()),
            NbtTag::String(text) => parse_compound(text).map_err(|_| self.bad()),
            _ => Err(self.bad()),
        }
    }

    fn suspicious_stew_effects(&self, tag: &NbtTag) -> Result<SuspiciousStewEffects, SavedError> {
        let list = self.list(tag)?;
        let mut effects: Vec<StewEffect> = Vec::with_capacity(list.len());
        for element in list {
            let map = self.map(element)?;
            effects.push(StewEffect {
                effect: self.mob_effect(self.necessary(map, "id")?)?,
                duration: (map.get("duration"))
                    .and_then(TagNumber::of)
                    .map_or(StewEffect::DEFAULT_DURATION, TagNumber::to_int),
            });
        }
        Ok(SuspiciousStewEffects(effects))
    }

    fn each<T>(
        &self,
        list: &[NbtTag],
        mut read: impl FnMut(&Self, &NbtTag) -> Result<T, SavedError>,
    ) -> Result<Vec<T>, SavedError> {
        let mut values: Vec<T> = Vec::with_capacity(list.len());
        for element in list {
            values.push(read(self, element)?);
        }
        Ok(values)
    }

    fn templates(&self, list: &[NbtTag]) -> Result<Vec<ItemStack>, SavedError> {
        let mut stacks: Vec<ItemStack> = Vec::new();
        for element in list {
            stacks.push(self.template(element, LIST_TEMPLATE_LEVELS)?);
        }
        Ok(stacks)
    }

    fn template(&self, tag: &NbtTag, levels: u32) -> Result<ItemStack, SavedError> {
        let depth = self.depth.saturating_add(levels);
        if depth > MAX_DEPTH {
            return Err(SavedError::TooDeep);
        }
        match tag {
            NbtTag::String(id) => Ok(ItemStack::new(read_item(id, self.names)?, Count::ONE)),
            NbtTag::Compound(map) => read_stack(map, self.names, depth),
            _ => Err(self.bad()),
        }
    }

    fn container(&self, tag: &NbtTag) -> Result<Container, SavedError> {
        let list = self.list(tag)?;
        if list.len() > Container::MAX_SLOTS {
            return Err(SavedError::OutOfRange(self.component));
        }
        let mut slots: Vec<Option<ItemStack>> = Vec::new();
        for element in list {
            let map = self.map(element)?;
            let slot = self.int(self.necessary(map, "slot")?, 0..=255)?;
            let slot = usize::try_from(slot).map_err(|_| SavedError::OutOfRange(self.component))?;
            let item = self.template(self.necessary(map, "item")?, SLOT_TEMPLATE_LEVELS)?;
            if slots.len() <= slot {
                slots.resize(slot.saturating_add(1), None);
            }
            if let Some(present) = slots.get_mut(slot) {
                *present = Some(item);
            }
        }
        Container::new(slots).ok_or(SavedError::OutOfRange(self.component))
    }

    fn entity_data(&self, tag: &NbtTag) -> Result<EntityData, SavedError> {
        let data = self.custom_data(tag)?;
        let entity_type = self.string(self.necessary(&data, ENTITY_TYPE_KEY)?)?;
        let entity_type = self.entity_type(entity_type)?;
        Ok(EntityData::new(entity_type, data))
    }

    fn bee(&self, tag: &NbtTag) -> Result<BeeOccupant, SavedError> {
        let map = self.map(tag)?;
        Ok(BeeOccupant {
            entity_data: self.entity_data(self.necessary(map, "entity_data")?)?,
            ticks_in_hive: self.number(self.necessary(map, "ticks_in_hive")?)?.to_int(),
            min_ticks_in_hive: self
                .number(self.necessary(map, "min_ticks_in_hive")?)?
                .to_int(),
        })
    }

    fn firework_explosion(&self, tag: &NbtTag) -> Result<FireworkExplosion, SavedError> {
        let map = self.map(tag)?;
        let shape = self.string(self.necessary(map, "shape")?)?;
        let int = |reader: &Self, tag: &NbtTag| Ok(reader.number(tag)?.to_int());
        Ok(FireworkExplosion {
            shape: FireworkShape::from_name(shape).ok_or(self.bad())?,
            colors: self.listed(map, "colors", int)?,
            fade_colors: self.listed(map, "fade_colors", int)?,
            has_trail: self.flag(map, "has_trail", false)?,
            has_twinkle: self.flag(map, "has_twinkle", false)?,
        })
    }

    fn fireworks(&self, tag: &NbtTag) -> Result<Fireworks, SavedError> {
        let map = self.map(tag)?;
        let explosions = match map.get("explosions") {
            Some(tag) => {
                let list = self.list(tag)?;
                if list.len() > Fireworks::MAX_EXPLOSIONS {
                    return Err(SavedError::OutOfRange(self.component));
                }
                self.each(list, Self::firework_explosion)?
            }
            None => Vec::new(),
        };
        let flight_duration = self.optional(map, "flight_duration", |reader, tag| {
            Ok(reader.number(tag)?.to_int() as u8)
        })?;
        Ok(Fireworks {
            flight_duration: flight_duration.unwrap_or_default(),
            explosions,
        })
    }

    fn banner_layer(&self, tag: &NbtTag) -> Result<BannerLayer, SavedError> {
        let map = self.map(tag)?;
        let pattern = match self.necessary(map, "pattern")? {
            NbtTag::Compound(pattern) => RegistryValue::Inline(InlineBannerPattern::new(
                self.identifier(self.necessary(pattern, "asset_id")?)?
                    .to_buf(),
                self.string(self.necessary(pattern, "translation_key")?)?
                    .to_string(),
            )),
            pattern => RegistryValue::Reference(BannerPatternId(
                self.entry(NameTable::BannerPattern, self.identifier(pattern)?)?,
            )),
        };
        let color = self.string(self.necessary(map, "color")?)?;
        Ok(BannerLayer {
            pattern,
            color: DyeColor::from_name(color).ok_or(self.bad())?,
        })
    }

    fn instrument(
        &self,
        tag: &NbtTag,
    ) -> Result<RegistryValue<InstrumentId, InlineInstrument>, SavedError> {
        let map = match tag {
            NbtTag::Compound(map) => map,
            _ => {
                let id = self.entry(NameTable::Instrument, self.identifier(tag)?)?;
                return Ok(RegistryValue::Reference(InstrumentId(id)));
            }
        };
        let use_duration = self.float(self.necessary(map, "use_duration")?)?;
        let range = self.float(self.necessary(map, "range")?)?;
        if !is_use_duration(use_duration) || !is_instrument_range(range) {
            return Err(SavedError::OutOfRange(self.component));
        }
        Ok(RegistryValue::Inline(InlineInstrument {
            sound_event: self.sound_event(self.necessary(map, "sound_event")?)?,
            use_duration,
            range,
            durability_damage: (self.optional(map, "durability_damage", |reader, tag| {
                reader.int(tag, NOT_NEGATIVE)
            })?)
            .unwrap_or_default(),
            description: read_text(self.necessary(map, "description")?, self.component)?,
        }))
    }

    fn piercing_weapon(&self, tag: &NbtTag) -> Result<PiercingWeapon, SavedError> {
        let map = self.map(tag)?;
        let mut weapon = PiercingWeapon::new();
        weapon.deals_knockback = self.flag(map, "deals_knockback", weapon.deals_knockback)?;
        weapon.dismounts = self.flag(map, "dismounts", weapon.dismounts)?;
        weapon.sound = self.optional(map, "sound", Self::sound_event)?;
        weapon.hit_sound = self.optional(map, "hit_sound", Self::sound_event)?;
        Ok(weapon)
    }

    fn swing_animation(&self, tag: &NbtTag) -> Result<SwingAnimation, SavedError> {
        let map = self.map(tag)?;
        let kind = match map.get("type") {
            Some(tag) => SwingKind::from_name(self.string(tag)?).ok_or(self.bad())?,
            None => SwingAnimation::DEFAULT_KIND,
        };
        let duration = match map.get("duration") {
            Some(tag) => self.int(tag, NOT_NEGATIVE)?,
            None => SwingAnimation::DEFAULT_DURATION,
        };
        Ok(SwingAnimation { kind, duration })
    }
}

fn write_value<L: NameLookup>(value: &Builtin, names: &L) -> Result<NbtTag, SavedError> {
    saved_form(value, names, &mut TagSink).map_err(Fault::into_saved)
}
