use alloc::borrow::Cow;
use alloc::string::String;
use alloc::vec::Vec;
use core::mem;
use core::num::NonZeroU8;

use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_nbt::tag::NbtTag;
use mcrs_minecraft_resource::ResourceLocation;
use mcrs_minecraft_resource::ResourceLocationBuf;
use mcrs_minecraft_text::Text;
use mcrs_minecraft_wire::Decode;
use mcrs_minecraft_wire::Encode;
use mcrs_minecraft_wire::VarInt;
use mcrs_voxel::BlockPos;

use crate::ArgbColor;
use crate::AttributeId;
use crate::AttributeModifier;
use crate::AttributeModifiers;
use crate::BlockId;
use crate::BlockSet;
use crate::BlockState;
use crate::BlockStateProperty;
use crate::BlockTagId;
use crate::Builtin;
use crate::BuiltinKey;
use crate::ComponentType;
use crate::CustomData;
use crate::CustomModelData;
use crate::CustomName;
use crate::Damage;
use crate::DimensionPos;
use crate::DyedColor;
use crate::EffectInstance;
use crate::EnchantmentGlintOverride;
use crate::EnchantmentId;
use crate::EnchantmentLevel;
use crate::Enchantments;
use crate::EntityTypeId;
use crate::EntityTypeSet;
use crate::EntityTypeTagId;
use crate::Equippable;
use crate::EquippableSlot;
use crate::Food;
use crate::InlineInstrument;
use crate::InlineSoundEvent;
use crate::InlineTrimMaterial;
use crate::InlineTrimPattern;
use crate::Instrument;
use crate::InstrumentId;
use crate::ItemModel;
use crate::ItemName;
use crate::LodestoneTracker;
use crate::Lore;
use crate::MapId;
use crate::MaxDamage;
use crate::MaxStackSize;
use crate::MobEffectId;
use crate::ModifierDisplay;
use crate::NameLookup;
use crate::NameTable;
use crate::OminousBottleAmplifier;
use crate::PotionContents;
use crate::PotionId;
use crate::Profile;
use crate::Rarity;
use crate::RegistryValue;
use crate::RepairCost;
use crate::SlotGroup;
use crate::SoundEventId;
use crate::SoundEventRef;
use crate::StewEffect;
use crate::SuspiciousStewEffects;
use crate::Tool;
use crate::ToolRule;
use crate::TooltipDisplay;
use crate::Trim;
use crate::TrimMaterialId;
use crate::TrimPatternId;
use crate::Unbreakable;
use crate::WireError;
use crate::lore::MAX_LINES;
use crate::saved;
use crate::saved_form::Fault;
use crate::saved_form::saved_form;
use crate::split_nbt::split_nbt;
use crate::tag_sink::TagSink;
use crate::wire_text::read_text;
use crate::wire_text::write_text;
use mcrs_minecraft_entity_attribute::ModifierOperation;

const DISPLAY_DEFAULT: i32 = 0;
const DISPLAY_HIDDEN: i32 = 1;
const DISPLAY_OVERRIDE: i32 = 2;

const HOLDER_SET_TAG: i32 = 0;
const INLINE_VALUE: i32 = 0;

pub(crate) fn put_var_int(output: &mut Vec<u8>, value: i32) {
    let _ = VarInt(value).encode(output);
}

fn write_registry_reference(output: &mut Vec<u8>, id: u16) {
    put_var_int(output, i32::from(id) + 1);
}

pub(crate) fn read_var_int(
    input: &mut &[u8],
    error: impl FnOnce(&mcrs_minecraft_wire::Error) -> WireError,
) -> Result<i32, WireError> {
    VarInt::decode(input)
        .map(|VarInt(value)| value)
        .map_err(|decode_error| error(&decode_error))
}

pub(crate) fn put_length(output: &mut Vec<u8>, length: usize) -> Result<(), WireError> {
    put_var_int(
        output,
        i32::try_from(length).map_err(|_| WireError::TooLong)?,
    );
    Ok(())
}

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
        BuiltinKey::MapId => Builtin::MapId(MapId(reader.var_int(input)?)),
        BuiltinKey::MapDecorations => reader.saved_form_as_nbt(input)?,
        BuiltinKey::Profile => Builtin::Profile(
            Profile::decode(input).map_err(|error| WireError::of_value(&error, component))?,
        ),
        BuiltinKey::Unbreakable => Builtin::Unbreakable(Unbreakable),
        BuiltinKey::ItemModel => {
            Builtin::ItemModel(ItemModel::new(reader.identifier(input)?.to_buf()))
        }
        BuiltinKey::Rarity => Builtin::Rarity(reader.list_entry(input, &Rarity::ALL)?),
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
        BuiltinKey::Equippable => Builtin::Equippable(reader.equippable(input)?),
        BuiltinKey::CustomName => Builtin::CustomName(CustomName(read_text(input, component)?)),
        BuiltinKey::ItemName => Builtin::ItemName(ItemName(read_text(input, component)?)),
        BuiltinKey::Lore => Builtin::Lore(reader.lore(input)?),
        BuiltinKey::DyedColor => Builtin::DyedColor(DyedColor(reader.color(input)?)),
        BuiltinKey::CustomModelData => Builtin::CustomModelData(CustomModelData {
            floats: reader.list(input, |reader, input| {
                Ok(f32::from_be_bytes(reader.bytes(input)?))
            })?,
            flags: reader.list(input, Reader::boolean)?,
            strings: reader.list(input, Reader::string)?,
            colors: reader.list(input, Reader::color)?,
        }),
        BuiltinKey::EnchantmentGlintOverride => {
            Builtin::EnchantmentGlintOverride(EnchantmentGlintOverride(reader.boolean(input)?))
        }
        BuiltinKey::BlockState => Builtin::BlockState(reader.block_state(input)?),
        BuiltinKey::LodestoneTracker => Builtin::LodestoneTracker(LodestoneTracker {
            target: reader.optional(input, Reader::global_pos)?,
            tracked: reader.boolean(input)?,
        }),
        BuiltinKey::Trim => Builtin::Trim(reader.trim(input)?),
        BuiltinKey::PotionContents => Builtin::PotionContents(reader.potion_contents(input)?),
        BuiltinKey::CustomData => Builtin::CustomData(CustomData(reader.custom_data(input)?)),
        BuiltinKey::SuspiciousStewEffects => Builtin::SuspiciousStewEffects(SuspiciousStewEffects(
            reader.list(input, |reader, input| {
                Ok(StewEffect {
                    effect: MobEffectId(reader.known_id(input, NameTable::MobEffect)?),
                    duration: reader.var_int(input)?,
                })
            })?,
        )),
        BuiltinKey::OminousBottleAmplifier => {
            Builtin::OminousBottleAmplifier(OminousBottleAmplifier(reader.var_int(input)?))
        }
        BuiltinKey::Instrument => Builtin::Instrument(Instrument(reader.instrument(input)?)),
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
        read_var_int(input, |error| WireError::of_value(error, self.component))
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

    fn known_id(&self, input: &mut &[u8], table: NameTable) -> Result<u16, WireError> {
        let id = self.var_int(input)?;
        self.named_id(id, table)
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

    fn saved_form_as_nbt(&self, input: &mut &[u8]) -> Result<Builtin, WireError> {
        let mut bytes = split_nbt(input, self.component)?;
        let tag = NbtTag::decode(&mut bytes)
            .map_err(|error| WireError::of_value(&error, self.component))?;
        saved::read_value(self.component, &tag, self.names)
            .map_err(|error| WireError::of_saved(error, self.component))
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
            let operation = u8::try_from(self.var_int(input)?)
                .ok()
                .and_then(|operation| ModifierOperation::try_from(operation).ok())
                .ok_or(self.bad())?;
            let mut modifier = AttributeModifier::new(attribute, id, amount, operation);
            modifier.slot = self.list_entry(input, &SlotGroup::ALL)?;
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
        if form == HOLDER_SET_TAG {
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

    fn equippable(&self, input: &mut &[u8]) -> Result<Equippable, WireError> {
        let mut equippable = Equippable::new(self.list_entry(input, &EquippableSlot::ALL)?);
        equippable.equip_sound = self.optional(input, Self::sound_event)?;
        equippable.set_asset_id(self.optional(input, Self::identifier_buf)?);
        equippable.set_camera_overlay(self.optional(input, Self::identifier_buf)?);
        equippable.allowed_entities = self.optional(input, Self::entity_type_set)?;
        equippable.dispensable = self.boolean(input)?;
        equippable.swappable = self.boolean(input)?;
        equippable.damage_on_hurt = self.boolean(input)?;
        equippable.equip_on_interact = self.boolean(input)?;
        equippable.can_be_sheared = self.boolean(input)?;
        equippable.shearing_sound = self.optional(input, Self::sound_event)?;
        Ok(equippable)
    }

    fn optional<T>(
        &self,
        input: &mut &[u8],
        read: impl FnOnce(&Self, &mut &[u8]) -> Result<T, WireError>,
    ) -> Result<Option<T>, WireError> {
        if self.boolean(input)? {
            read(self, input).map(Some)
        } else {
            Ok(None)
        }
    }

    fn identifier_buf(&self, input: &mut &[u8]) -> Result<ResourceLocationBuf, WireError> {
        Ok(self.identifier(input)?.to_buf())
    }

    fn named_id(&self, id: i32, table: NameTable) -> Result<u16, WireError> {
        u16::try_from(id)
            .ok()
            .filter(|id| self.names.name(table, *id).is_some())
            .ok_or(WireError::UnknownEntry {
                component: self.component,
                table,
            })
    }

    fn sound_event(&self, input: &mut &[u8]) -> Result<SoundEventRef, WireError> {
        let Some(id) = self.registry_form(input, NameTable::SoundEvent)? else {
            let mut inline = InlineSoundEvent::new(self.identifier_buf(input)?);
            inline.range = self.optional(input, |reader, input| {
                Ok(f32::from_be_bytes(reader.bytes(input)?))
            })?;
            return Ok(SoundEventRef::Inline(inline));
        };
        Ok(SoundEventRef::Reference(SoundEventId(id)))
    }

    fn entity_type_set(&self, input: &mut &[u8]) -> Result<EntityTypeSet, WireError> {
        let form = self.var_int(input)?;
        if form == HOLDER_SET_TAG {
            let name = self.identifier(input)?;
            let entity_tag =
                self.names
                    .id(NameTable::EntityTypeTag, name)
                    .ok_or(WireError::UnknownEntry {
                        component: self.component,
                        table: NameTable::EntityTypeTag,
                    })?;
            return Ok(EntityTypeSet::Tag(EntityTypeTagId(entity_tag)));
        }
        let count = usize::try_from(form)
            .ok()
            .and_then(|form| form.checked_sub(1))
            .ok_or(self.bad())?;
        let mut entity_types: Vec<EntityTypeId> = Vec::with_capacity(count.min(input.len()));
        for _ in 0..count {
            let id = self.var_int(input)?;
            entity_types.push(EntityTypeId(self.named_id(id, NameTable::EntityType)?));
        }
        Ok(EntityTypeSet::EntityTypes(Cow::Owned(entity_types)))
    }

    fn color(&self, input: &mut &[u8]) -> Result<ArgbColor, WireError> {
        Ok(ArgbColor(i32::from_be_bytes(self.bytes(input)?)))
    }

    fn string(&self, input: &mut &[u8]) -> Result<String, WireError> {
        String::decode(input).map_err(|error| WireError::of_value(&error, self.component))
    }

    fn list<T>(
        &self,
        input: &mut &[u8],
        mut read: impl FnMut(&Self, &mut &[u8]) -> Result<T, WireError>,
    ) -> Result<Vec<T>, WireError> {
        let count = self.length(input)?;
        let budget = input.len() / mem::size_of::<T>().max(1);
        let mut values: Vec<T> = Vec::with_capacity(count.min(budget));
        for _ in 0..count {
            values.push(read(self, input)?);
        }
        Ok(values)
    }

    fn block_state(&self, input: &mut &[u8]) -> Result<BlockState, WireError> {
        let properties = self.list(input, |reader, input| {
            Ok(BlockStateProperty {
                name: reader.string(input)?,
                value: reader.string(input)?,
            })
        })?;
        Ok(BlockState::new(properties))
    }

    fn global_pos(&self, input: &mut &[u8]) -> Result<DimensionPos, WireError> {
        let dimension = self.identifier_buf(input)?;
        let pos =
            BlockPos::decode(input).map_err(|error| WireError::of_value(&error, self.component))?;
        Ok(DimensionPos::new(dimension, pos))
    }

    fn registry_form(&self, input: &mut &[u8], table: NameTable) -> Result<Option<u16>, WireError> {
        let form = self.var_int(input)?;
        if form == INLINE_VALUE {
            return Ok(None);
        }
        let id = form
            .checked_sub(1)
            .filter(|id| *id >= 0)
            .ok_or(self.bad())?;
        self.named_id(id, table).map(Some)
    }

    fn trim(&self, input: &mut &[u8]) -> Result<Trim, WireError> {
        let material = match self.registry_form(input, NameTable::TrimMaterial)? {
            Some(id) => RegistryValue::Reference(TrimMaterialId(id)),
            None => RegistryValue::Inline(InlineTrimMaterial::new(
                self.identifier_buf(input)?,
                read_text(input, self.component)?,
            )),
        };
        let pattern = match self.registry_form(input, NameTable::TrimPattern)? {
            Some(id) => RegistryValue::Reference(TrimPatternId(id)),
            None => {
                let mut inline = InlineTrimPattern::new(
                    self.identifier_buf(input)?,
                    read_text(input, self.component)?,
                );
                inline.decal = self.boolean(input)?;
                RegistryValue::Inline(inline)
            }
        };
        Ok(Trim { material, pattern })
    }

    fn potion_contents(&self, input: &mut &[u8]) -> Result<PotionContents, WireError> {
        Ok(PotionContents {
            potion: self.optional(input, |reader, input| {
                Ok(PotionId(reader.known_id(input, NameTable::Potion)?))
            })?,
            custom_color: self.optional(input, |reader, input| {
                Ok(i32::from_be_bytes(reader.bytes(input)?))
            })?,
            custom_effects: self.list(input, Self::effect_instance)?,
            custom_name: self.optional(input, Self::string)?,
        })
    }

    fn effect_instance(&self, input: &mut &[u8]) -> Result<EffectInstance, WireError> {
        let mut effect =
            EffectInstance::new(MobEffectId(self.known_id(input, NameTable::MobEffect)?));
        self.effect_details(input, &mut effect)?;
        let mut hidden = EffectInstance::new(effect.effect);
        while self.boolean(input)? {
            self.effect_details(input, &mut hidden)?;
        }
        Ok(effect)
    }

    fn effect_details(
        &self,
        input: &mut &[u8],
        effect: &mut EffectInstance,
    ) -> Result<(), WireError> {
        effect.amplifier = u8::try_from(self.var_int(input)?.clamp(0, 255)).unwrap_or(u8::MAX);
        effect.duration = self.var_int(input)?;
        effect.ambient = self.boolean(input)?;
        effect.show_particles = self.boolean(input)?;
        effect.show_icon = self.boolean(input)?;
        Ok(())
    }

    fn custom_data(&self, input: &mut &[u8]) -> Result<NbtCompound, WireError> {
        let mut bytes = split_nbt(input, self.component)?;
        match NbtTag::decode(&mut bytes) {
            Ok(NbtTag::Compound(compound)) => Ok(compound),
            Ok(_) => Err(self.bad()),
            Err(error) => Err(WireError::of_value(&error, self.component)),
        }
    }

    fn instrument(
        &self,
        input: &mut &[u8],
    ) -> Result<RegistryValue<InstrumentId, InlineInstrument>, WireError> {
        if let Some(id) = self.registry_form(input, NameTable::Instrument)? {
            return Ok(RegistryValue::Reference(InstrumentId(id)));
        }
        Ok(RegistryValue::Inline(InlineInstrument {
            sound_event: self.sound_event(input)?,
            use_duration: f32::from_be_bytes(self.bytes(input)?),
            range: f32::from_be_bytes(self.bytes(input)?),
            durability_damage: self.var_int(input)?,
            description: read_text(input, self.component)?,
        }))
    }
}

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
        | Builtin::RepairCost(RepairCost(number))
        | Builtin::MapId(MapId(number)) => put_var_int(output, *number),
        Builtin::Unbreakable(Unbreakable) => {}
        Builtin::ItemModel(model) => writer.identifier(model.get(), output)?,
        Builtin::Rarity(rarity) => writer.list_id(&Rarity::ALL, *rarity, output)?,
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
        Builtin::Equippable(equippable) => writer.equippable(equippable, output)?,
        Builtin::MapDecorations(_) => writer.saved_form_as_nbt(value, output)?,
        Builtin::Profile(profile) => profile
            .encode(&mut *output)
            .map_err(|_| WireError::BadValue(writer.component))?,
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
        Builtin::DyedColor(DyedColor(color)) => output.extend_from_slice(&color.0.to_be_bytes()),
        Builtin::CustomModelData(data) => writer.custom_model_data(data, output)?,
        Builtin::EnchantmentGlintOverride(EnchantmentGlintOverride(value)) => {
            put_boolean(output, *value);
        }
        Builtin::BlockState(state) => {
            writer.list(state.properties(), output, |writer, property, output| {
                writer.string(&property.name, output)?;
                writer.string(&property.value, output)
            })?;
        }
        Builtin::LodestoneTracker(tracker) => {
            writer.optional(tracker.target.as_ref(), output, |writer, target, output| {
                writer.identifier(target.dimension(), output)?;
                target
                    .pos
                    .encode(&mut *output)
                    .map_err(|_| WireError::OutOfRange(writer.component))
            })?;
            put_boolean(output, tracker.tracked);
        }
        Builtin::Trim(trim) => writer.trim(trim, output)?,
        Builtin::PotionContents(contents) => writer.potion_contents(contents, output)?,
        Builtin::CustomData(CustomData(compound)) => compound
            .encode(&mut *output)
            .map_err(|_| WireError::BadValue(writer.component))?,
        Builtin::SuspiciousStewEffects(SuspiciousStewEffects(effects)) => {
            writer.list(effects, output, |_, effect, output| {
                put_var_int(output, i32::from(effect.effect.0));
                put_var_int(output, effect.duration);
                Ok(())
            })?;
        }
        Builtin::OminousBottleAmplifier(OminousBottleAmplifier(amplifier)) => {
            put_var_int(output, *amplifier);
        }
        Builtin::Instrument(Instrument(instrument)) => writer.instrument(instrument, output)?,
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

    fn saved_form_as_nbt(&self, value: &Builtin, output: &mut Vec<u8>) -> Result<(), WireError> {
        let tag = saved_form(value, self.names, &mut TagSink).map_err(Fault::into_wire)?;
        tag.encode(&mut *output)
            .map_err(|_| WireError::BadValue(self.component))
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
            put_var_int(output, i32::from(u8::from(modifier.operation)));
            self.list_id(&SlotGroup::ALL, modifier.slot, output)?;
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
                put_var_int(output, HOLDER_SET_TAG);
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

    fn equippable(&self, equippable: &Equippable, output: &mut Vec<u8>) -> Result<(), WireError> {
        self.list_id(&EquippableSlot::ALL, equippable.slot, output)?;
        self.optional(equippable.equip_sound.as_ref(), output, Self::sound_event)?;
        self.optional(equippable.asset_id(), output, Self::identifier)?;
        self.optional(equippable.camera_overlay(), output, Self::identifier)?;
        self.optional(
            equippable.allowed_entities.as_ref(),
            output,
            Self::entity_type_set,
        )?;
        put_boolean(output, equippable.dispensable);
        put_boolean(output, equippable.swappable);
        put_boolean(output, equippable.damage_on_hurt);
        put_boolean(output, equippable.equip_on_interact);
        put_boolean(output, equippable.can_be_sheared);
        self.optional(
            equippable.shearing_sound.as_ref(),
            output,
            Self::sound_event,
        )
    }

    fn optional<T>(
        &self,
        value: Option<T>,
        output: &mut Vec<u8>,
        write: impl FnOnce(&Self, T, &mut Vec<u8>) -> Result<(), WireError>,
    ) -> Result<(), WireError> {
        put_boolean(output, value.is_some());
        value.map_or(Ok(()), |value| write(self, value, output))
    }

    fn sound_event(&self, sound: &SoundEventRef, output: &mut Vec<u8>) -> Result<(), WireError> {
        match sound {
            SoundEventRef::Reference(id) => {
                write_registry_reference(output, id.0);
                Ok(())
            }
            SoundEventRef::Inline(inline) => {
                put_var_int(output, INLINE_VALUE);
                self.identifier(inline.sound_id(), output)?;
                self.optional(inline.range, output, |_, range, output| {
                    output.extend_from_slice(&range.to_be_bytes());
                    Ok(())
                })
            }
        }
    }

    fn string(&self, value: &str, output: &mut Vec<u8>) -> Result<(), WireError> {
        value
            .encode(&mut *output)
            .map_err(|_| WireError::OutOfRange(self.component))
    }

    fn list<T>(
        &self,
        values: &[T],
        output: &mut Vec<u8>,
        mut write: impl FnMut(&Self, &T, &mut Vec<u8>) -> Result<(), WireError>,
    ) -> Result<(), WireError> {
        put_length(output, values.len())?;
        for value in values {
            write(self, value, output)?;
        }
        Ok(())
    }

    fn custom_model_data(
        &self,
        data: &CustomModelData,
        output: &mut Vec<u8>,
    ) -> Result<(), WireError> {
        self.list(&data.floats, output, |_, value, output| {
            output.extend_from_slice(&value.to_be_bytes());
            Ok(())
        })?;
        self.list(&data.flags, output, |_, value, output| {
            put_boolean(output, *value);
            Ok(())
        })?;
        self.list(&data.strings, output, |writer, value, output| {
            writer.string(value, output)
        })?;
        self.list(&data.colors, output, |_, color, output| {
            output.extend_from_slice(&color.0.to_be_bytes());
            Ok(())
        })
    }

    fn trim(&self, trim: &Trim, output: &mut Vec<u8>) -> Result<(), WireError> {
        match &trim.material {
            RegistryValue::Reference(id) => write_registry_reference(output, id.0),
            RegistryValue::Inline(material) => {
                put_var_int(output, INLINE_VALUE);
                self.identifier(material.palette_id(), output)?;
                write_text(&material.description, self.component, output)?;
            }
        }
        match &trim.pattern {
            RegistryValue::Reference(id) => write_registry_reference(output, id.0),
            RegistryValue::Inline(pattern) => {
                put_var_int(output, INLINE_VALUE);
                self.identifier(pattern.asset_id(), output)?;
                write_text(&pattern.description, self.component, output)?;
                put_boolean(output, pattern.decal);
            }
        }
        Ok(())
    }

    fn potion_contents(
        &self,
        contents: &PotionContents,
        output: &mut Vec<u8>,
    ) -> Result<(), WireError> {
        self.optional(contents.potion, output, |_, potion, output| {
            put_var_int(output, i32::from(potion.0));
            Ok(())
        })?;
        self.optional(contents.custom_color, output, |_, color, output| {
            output.extend_from_slice(&color.to_be_bytes());
            Ok(())
        })?;
        self.list(&contents.custom_effects, output, |_, effect, output| {
            put_var_int(output, i32::from(effect.effect.0));
            put_var_int(output, i32::from(effect.amplifier));
            put_var_int(output, effect.duration);
            put_boolean(output, effect.ambient);
            put_boolean(output, effect.show_particles);
            put_boolean(output, effect.show_icon);
            put_boolean(output, false);
            Ok(())
        })?;
        self.optional(contents.custom_name.as_deref(), output, Self::string)
    }

    fn instrument(
        &self,
        instrument: &RegistryValue<InstrumentId, InlineInstrument>,
        output: &mut Vec<u8>,
    ) -> Result<(), WireError> {
        match instrument {
            RegistryValue::Reference(id) => {
                write_registry_reference(output, id.0);
                Ok(())
            }
            RegistryValue::Inline(inline) => {
                put_var_int(output, INLINE_VALUE);
                self.sound_event(&inline.sound_event, output)?;
                output.extend_from_slice(&inline.use_duration.to_be_bytes());
                output.extend_from_slice(&inline.range.to_be_bytes());
                put_var_int(output, inline.durability_damage);
                write_text(&inline.description, self.component, output)
            }
        }
    }

    fn entity_type_set(
        &self,
        entity_types: &EntityTypeSet,
        output: &mut Vec<u8>,
    ) -> Result<(), WireError> {
        match entity_types {
            EntityTypeSet::Tag(entity_tag) => {
                let name = self
                    .names
                    .name(NameTable::EntityTypeTag, entity_tag.0)
                    .ok_or(WireError::UnnamedId(NameTable::EntityTypeTag))?;
                put_var_int(output, HOLDER_SET_TAG);
                self.identifier(name, output)
            }
            EntityTypeSet::EntityTypes(entity_types) => {
                let form = entity_types
                    .len()
                    .checked_add(1)
                    .ok_or(WireError::TooLong)?;
                put_length(output, form)?;
                for entity_type in entity_types.iter() {
                    put_var_int(output, i32::from(entity_type.0));
                }
                Ok(())
            }
        }
    }
}
