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
use crate::Lore;
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
use crate::PiercingWeapon;
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
use crate::WireError;
use crate::check_saved_form::MAX_COUNT;
use crate::lore::MAX_LINES;
use crate::nbt_depth::LIST_TEMPLATE_LEVELS;
use crate::nbt_depth::SLOT_TEMPLATE_LEVELS;
use crate::saved;
use crate::saved::is_air;
use crate::saved_form::Fault;
use crate::saved_form::saved_form;
use crate::split_nbt::split_nbt;
use crate::tag_sink::TagSink;
use crate::wire::Entries;
use crate::wire::read_patch;
use crate::wire::write_patch;
use crate::wire_text::read_text;
use crate::wire_text::write_text;
use mcrs_minecraft_entity_attribute::ModifierOperation;
use mcrs_minecraft_nbt::MAX_DEPTH;

const DISPLAY_DEFAULT: i32 = 0;
const DISPLAY_HIDDEN: i32 = 1;
const DISPLAY_OVERRIDE: i32 = 2;

const HOLDER_SET_TAG: i32 = 0;
const INLINE_VALUE: i32 = 0;

pub(crate) fn put_var_int(output: &mut Vec<u8>, value: i32) {
    let _ = VarInt(value).encode(output);
}

fn write_enchantments(enchantments: &Enchantments, output: &mut Vec<u8>) -> Result<(), WireError> {
    put_length(output, enchantments.entries().len())?;
    for entry in enchantments.entries() {
        put_var_int(output, i32::from(entry.enchantment.0));
        put_var_int(output, i32::from(entry.level.get()));
    }
    Ok(())
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

pub(crate) fn read_value_at<L: NameLookup>(
    component: BuiltinKey,
    input: &mut &[u8],
    names: &L,
    depth: u32,
) -> Result<Builtin, WireError> {
    let reader = Reader {
        component,
        names,
        depth,
    };
    match component {
        BuiltinKey::ChargedProjectiles => reader
            .list_templates(input, ChargedProjectiles::MAX_PROJECTILES)
            .map(|stacks| Builtin::ChargedProjectiles(ChargedProjectiles(stacks))),
        BuiltinKey::BundleContents => reader
            .list_templates(input, usize::MAX)
            .map(|stacks| Builtin::BundleContents(BundleContents(stacks))),
        BuiltinKey::Container => reader.container(input).map(Builtin::Container),
        _ => read_flat_value(&reader, input),
    }
}

#[inline(never)]
fn read_flat_value<L: NameLookup>(
    reader: &Reader<'_, L>,
    input: &mut &[u8],
) -> Result<Builtin, WireError> {
    let component = reader.component;
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
        BuiltinKey::StoredEnchantments => {
            Builtin::StoredEnchantments(StoredEnchantments(reader.enchantments(input)?))
        }
        BuiltinKey::PaintingVariant => Builtin::PaintingVariant(reader.painting_variant(input)?),
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
        BuiltinKey::PiercingWeapon => Builtin::PiercingWeapon(reader.piercing_weapon(input)?),
        BuiltinKey::AttackAnimation => {
            Builtin::AttackAnimation(AttackAnimation(reader.swing_animation(input)?))
        }
        BuiltinKey::InteractAnimation => {
            Builtin::InteractAnimation(InteractAnimation(reader.swing_animation(input)?))
        }
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
        BuiltinKey::ChargedProjectiles | BuiltinKey::BundleContents | BuiltinKey::Container => {
            return read_value_at(component, input, reader.names, reader.depth);
        }
        BuiltinKey::FireworkExplosion => {
            Builtin::FireworkExplosion(reader.firework_explosion(input)?)
        }
        BuiltinKey::Fireworks => Builtin::Fireworks(Fireworks {
            flight_duration: u8::try_from(reader.var_int(input)?)
                .map_err(|_| WireError::OutOfRange(component))?,
            explosions: reader.limited_list(
                input,
                Fireworks::MAX_EXPLOSIONS,
                Reader::firework_explosion,
            )?,
        }),
        BuiltinKey::BannerPatterns => {
            Builtin::BannerPatterns(BannerPatterns(reader.list(input, Reader::banner_layer)?))
        }
        BuiltinKey::Bees => Builtin::Bees(Bees(reader.list(input, Reader::bee)?)),
        BuiltinKey::ContainerLoot => reader.saved_form_as_nbt(input)?,
    })
}

struct Reader<'names, L> {
    component: BuiltinKey,
    names: &'names L,
    depth: u32,
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
        let tag = mcrs_minecraft_nbt::without_nbt_budget(|| NbtTag::decode(&mut bytes))
            .map_err(|error| WireError::of_value(&error, self.component))?;
        saved::read_value(self.component, &tag, self.names)
            .map_err(|error| WireError::of_saved(error, self.component))
    }

    fn painting_variant(&self, input: &mut &[u8]) -> Result<PaintingVariant, WireError> {
        let id = self.registry_form(input, NameTable::PaintingVariant)?;
        Ok(PaintingVariant(PaintingVariantId(id.ok_or(self.bad())?)))
    }

    fn container(&self, input: &mut &[u8]) -> Result<Container, WireError> {
        let slots = self.templates(input, Container::MAX_SLOTS, |reader, input| {
            reader.optional(input, |reader, input| {
                reader.template(input, SLOT_TEMPLATE_LEVELS)
            })
        })?;
        Container::new(slots).ok_or(WireError::OutOfRange(self.component))
    }

    fn enchantments(&self, input: &mut &[u8]) -> Result<Enchantments, WireError> {
        let count = self.length(input)?;
        let mut entries: Vec<EnchantmentLevel> = Vec::with_capacity(count.min(input.len()));
        for _ in 0..count {
            let enchantment = EnchantmentId(self.known_id(input, NameTable::Enchantment)?);
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

    fn piercing_weapon(&self, input: &mut &[u8]) -> Result<PiercingWeapon, WireError> {
        let deals_knockback = self.boolean(input)?;
        let dismounts = self.boolean(input)?;
        let sound = self.optional(input, Self::sound_event)?;
        let hit_sound = self.optional(input, Self::sound_event)?;
        Ok(PiercingWeapon {
            deals_knockback,
            dismounts,
            sound,
            hit_sound,
        })
    }

    fn swing_animation(&self, input: &mut &[u8]) -> Result<SwingAnimation, WireError> {
        let kind = self.list_entry(input, &SwingKind::ALL)?;
        let duration = self.var_int(input)?;
        Ok(SwingAnimation { kind, duration })
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
        read: impl FnMut(&Self, &mut &[u8]) -> Result<T, WireError>,
    ) -> Result<Vec<T>, WireError> {
        let count = self.length(input)?;
        let budget = input.len() / mem::size_of::<T>().max(1);
        self.elements(input, count, Vec::with_capacity(count.min(budget)), read)
    }

    fn templates<T>(
        &self,
        input: &mut &[u8],
        max: usize,
        read: impl FnMut(&Self, &mut &[u8]) -> Result<T, WireError>,
    ) -> Result<Vec<T>, WireError> {
        let count = self.length(input)?;
        if count > max {
            return Err(WireError::OutOfRange(self.component));
        }
        self.elements(input, count, Vec::new(), read)
    }

    fn elements<T>(
        &self,
        input: &mut &[u8],
        count: usize,
        mut values: Vec<T>,
        mut read: impl FnMut(&Self, &mut &[u8]) -> Result<T, WireError>,
    ) -> Result<Vec<T>, WireError> {
        for _ in 0..count {
            values.push(read(self, input)?);
        }
        Ok(values)
    }

    fn limited_list<T>(
        &self,
        input: &mut &[u8],
        max: usize,
        read: impl FnMut(&Self, &mut &[u8]) -> Result<T, WireError>,
    ) -> Result<Vec<T>, WireError> {
        let mut peek = *input;
        if self.length(&mut peek)? > max {
            return Err(WireError::OutOfRange(self.component));
        }
        self.list(input, read)
    }

    fn list_templates(&self, input: &mut &[u8], max: usize) -> Result<Vec<ItemStack>, WireError> {
        self.templates(input, max, |reader, input| {
            reader.template(input, LIST_TEMPLATE_LEVELS)
        })
    }

    fn template(&self, input: &mut &[u8], levels: u32) -> Result<ItemStack, WireError> {
        let depth = self.depth.saturating_add(levels);
        if depth > MAX_DEPTH {
            return Err(WireError::TooDeep);
        }
        let kind = u16::try_from(self.var_int(input)?).map_err(|_| WireError::UnknownItem)?;
        let item = self
            .names
            .name(NameTable::Item, kind)
            .ok_or(WireError::UnknownItem)?;
        let air = is_air(item);
        let count = self.var_int(input)?;
        let mut stack = ItemStack::new(Kind(kind), Count::ONE);
        read_patch(input, self.names, Entries::Bare, depth, &mut stack)?;
        if air {
            return Err(WireError::EmptyStack);
        }
        stack.count = match u8::try_from(count) {
            Ok(count) => Count::new(count).ok_or(WireError::EmptyStack)?,
            Err(_) if count < 0 => return Err(self.bad()),
            Err(_) => return Err(WireError::CountTooLarge),
        };
        Ok(stack)
    }

    fn firework_explosion(&self, input: &mut &[u8]) -> Result<FireworkExplosion, WireError> {
        let int = |reader: &Self, input: &mut &[u8]| Ok(i32::from_be_bytes(reader.bytes(input)?));
        Ok(FireworkExplosion {
            shape: self.list_entry(input, &FireworkShape::ALL)?,
            colors: self.list(input, int)?,
            fade_colors: self.list(input, int)?,
            has_trail: self.boolean(input)?,
            has_twinkle: self.boolean(input)?,
        })
    }

    fn banner_layer(&self, input: &mut &[u8]) -> Result<BannerLayer, WireError> {
        let pattern = match self.registry_form(input, NameTable::BannerPattern)? {
            Some(id) => RegistryValue::Reference(BannerPatternId(id)),
            None => RegistryValue::Inline(InlineBannerPattern::new(
                self.identifier_buf(input)?,
                self.string(input)?,
            )),
        };
        Ok(BannerLayer {
            pattern,
            color: self.list_entry(input, &DyeColor::ALL)?,
        })
    }

    fn bee(&self, input: &mut &[u8]) -> Result<BeeOccupant, WireError> {
        let entity_type = EntityTypeId(self.known_id(input, NameTable::EntityType)?);
        Ok(BeeOccupant {
            entity_data: EntityData::new(entity_type, self.custom_data(input)?),
            ticks_in_hive: self.var_int(input)?,
            min_ticks_in_hive: self.var_int(input)?,
        })
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
        let mut bytes = mcrs_minecraft_nbt::split_wire(input)
            .map_err(|error| WireError::of_value(&error, self.component))?;
        match mcrs_minecraft_nbt::without_nbt_budget(|| NbtTag::decode(&mut bytes)) {
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

#[expect(
    clippy::too_many_lines,
    reason = "one dispatch arm per component wire form"
)]
pub(crate) fn write_value<L: NameLookup>(
    value: &Builtin,
    names: &L,
    depth: u32,
    output: &mut Vec<u8>,
) -> Result<(), WireError> {
    let writer = Writer {
        component: value.key(),
        names,
        depth,
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
        Builtin::Enchantments(enchantments)
        | Builtin::StoredEnchantments(StoredEnchantments(enchantments)) => {
            write_enchantments(enchantments, output)?;
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
        Builtin::PiercingWeapon(weapon) => writer.piercing_weapon(weapon, output)?,
        Builtin::AttackAnimation(AttackAnimation(swing))
        | Builtin::InteractAnimation(InteractAnimation(swing)) => {
            writer.swing_animation(swing, output)?;
        }
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
        Builtin::PaintingVariant(variant) => write_registry_reference(output, variant.0.0),
        Builtin::Instrument(Instrument(instrument)) => writer.instrument(instrument, output)?,
        Builtin::ChargedProjectiles(value) => writer.charged_projectiles(value, output)?,
        Builtin::BundleContents(value) => writer.list(&value.0, output, Writer::list_template)?,
        Builtin::FireworkExplosion(explosion) => writer.firework_explosion(explosion, output)?,
        Builtin::Fireworks(fireworks) => writer.fireworks(fireworks, output)?,
        Builtin::BannerPatterns(value) => writer.list(&value.0, output, Writer::banner_layer)?,
        Builtin::Container(container) => writer.container(container, output)?,
        Builtin::Bees(value) => writer.list(&value.0, output, Writer::bee)?,
        Builtin::ContainerLoot(_) => writer.saved_form_as_nbt(value, output)?,
    }
    Ok(())
}

fn put_boolean(output: &mut Vec<u8>, value: bool) {
    output.push(u8::from(value));
}

struct Writer<'names, L> {
    component: BuiltinKey,
    names: &'names L,
    depth: u32,
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

    fn list_template(&self, stack: &ItemStack, output: &mut Vec<u8>) -> Result<(), WireError> {
        self.template(stack, output, LIST_TEMPLATE_LEVELS)
    }

    fn template(
        &self,
        stack: &ItemStack,
        output: &mut Vec<u8>,
        levels: u32,
    ) -> Result<(), WireError> {
        let depth = self.depth.saturating_add(levels);
        if depth > MAX_DEPTH {
            return Err(WireError::TooDeep);
        }
        let item = self
            .names
            .name(NameTable::Item, stack.kind.0)
            .ok_or(WireError::UnnamedId(NameTable::Item))?;
        if is_air(item) {
            return Err(WireError::EmptyStack);
        }
        let count = stack.count.get();
        if count > MAX_COUNT {
            return Err(WireError::CountOutOfRange);
        }
        put_var_int(output, i32::from(stack.kind.0));
        put_var_int(output, i32::from(count));
        write_patch(stack, self.names, Entries::Bare, depth, output)
    }

    fn charged_projectiles(
        &self,
        ChargedProjectiles(projectiles): &ChargedProjectiles,
        output: &mut Vec<u8>,
    ) -> Result<(), WireError> {
        self.limited_list(
            projectiles,
            ChargedProjectiles::MAX_PROJECTILES,
            output,
            Self::list_template,
        )
    }

    fn container(&self, container: &Container, output: &mut Vec<u8>) -> Result<(), WireError> {
        self.list(container.slots(), output, |writer, slot, output| {
            writer.optional(slot.as_ref(), output, |writer, stack, output| {
                writer.template(stack, output, SLOT_TEMPLATE_LEVELS)
            })
        })
    }

    fn limited_list<T>(
        &self,
        values: &[T],
        max: usize,
        output: &mut Vec<u8>,
        write: impl FnMut(&Self, &T, &mut Vec<u8>) -> Result<(), WireError>,
    ) -> Result<(), WireError> {
        if values.len() > max {
            return Err(WireError::OutOfRange(self.component));
        }
        self.list(values, output, write)
    }

    fn fireworks(&self, fireworks: &Fireworks, output: &mut Vec<u8>) -> Result<(), WireError> {
        put_var_int(output, i32::from(fireworks.flight_duration));
        self.limited_list(
            &fireworks.explosions,
            Fireworks::MAX_EXPLOSIONS,
            output,
            Self::firework_explosion,
        )
    }

    fn bee(&self, occupant: &BeeOccupant, output: &mut Vec<u8>) -> Result<(), WireError> {
        put_var_int(output, i32::from(occupant.entity_data.entity_type.0));
        occupant
            .entity_data
            .data()
            .encode(&mut *output)
            .map_err(|_| WireError::BadValue(self.component))?;
        put_var_int(output, occupant.ticks_in_hive);
        put_var_int(output, occupant.min_ticks_in_hive);
        Ok(())
    }

    fn firework_explosion(
        &self,
        explosion: &FireworkExplosion,
        output: &mut Vec<u8>,
    ) -> Result<(), WireError> {
        self.list_id(&FireworkShape::ALL, explosion.shape, output)?;
        for colors in [&explosion.colors, &explosion.fade_colors] {
            self.list(colors, output, |_, color, output| {
                output.extend_from_slice(&color.to_be_bytes());
                Ok(())
            })?;
        }
        put_boolean(output, explosion.has_trail);
        put_boolean(output, explosion.has_twinkle);
        Ok(())
    }

    fn banner_layer(&self, layer: &BannerLayer, output: &mut Vec<u8>) -> Result<(), WireError> {
        match &layer.pattern {
            RegistryValue::Reference(id) => write_registry_reference(output, id.0),
            RegistryValue::Inline(pattern) => {
                put_var_int(output, INLINE_VALUE);
                self.identifier(pattern.asset_id(), output)?;
                self.string(&pattern.translation_key, output)?;
            }
        }
        self.list_id(&DyeColor::ALL, layer.color, output)
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

    fn piercing_weapon(
        &self,
        weapon: &PiercingWeapon,
        output: &mut Vec<u8>,
    ) -> Result<(), WireError> {
        put_boolean(output, weapon.deals_knockback);
        put_boolean(output, weapon.dismounts);
        self.optional(weapon.sound.as_ref(), output, Self::sound_event)?;
        self.optional(weapon.hit_sound.as_ref(), output, Self::sound_event)
    }

    fn swing_animation(
        &self,
        swing: &SwingAnimation,
        output: &mut Vec<u8>,
    ) -> Result<(), WireError> {
        self.list_id(&SwingKind::ALL, swing.kind, output)?;
        put_var_int(output, swing.duration);
        Ok(())
    }
}
