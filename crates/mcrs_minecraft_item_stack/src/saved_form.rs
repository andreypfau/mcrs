use core::ops::RangeInclusive;

use mcrs_minecraft_resource::ResourceLocation;
use mcrs_minecraft_text::Text;

use crate::AttackAnimation;
use crate::AttributeModifier;
use crate::AttributeModifiers;
use crate::BannerLayer;
use crate::BannerPatterns;
use crate::Bees;
use crate::BlockSet;
use crate::Builtin;
use crate::BuiltinKey;
use crate::BundleContents;
use crate::ChargedProjectiles;
use crate::ComponentType;
use crate::Container;
use crate::ContainerLoot;
use crate::CustomData;
use crate::CustomModelData;
use crate::CustomName;
use crate::Damage;
use crate::DyedColor;
use crate::EffectInstance;
use crate::EnchantmentGlintOverride;
use crate::Enchantments;
use crate::EntityData;
use crate::EntityTypeSet;
use crate::Equippable;
use crate::FireworkExplosion;
use crate::Fireworks;
use crate::Food;
use crate::HashError;
use crate::InlineInstrument;
use crate::Instrument;
use crate::InstrumentId;
use crate::InteractAnimation;
use crate::ItemName;
use crate::ItemStack;
use crate::LodestoneTracker;
use crate::Lore;
use crate::MapDecorations;
use crate::MapId;
use crate::MaxDamage;
use crate::MaxStackSize;
use crate::ModifierDisplay;
use crate::NameLookup;
use crate::NameTable;
use crate::OminousBottleAmplifier;
use crate::PaintingVariant;
use crate::PaintingVariantId;
use crate::PatchEntry;
use crate::PiercingWeapon;
use crate::PotionContents;
use crate::RegistryValue;
use crate::RepairCost;
use crate::SavedError;
use crate::SlotGroup;
use crate::SoundEventRef;
use crate::StewEffect;
use crate::StoredEnchantments;
use crate::SuspiciousStewEffects;
use crate::SwingAnimation;
use crate::Tool;
use crate::ToolRule;
use crate::TooltipDisplay;
use crate::Trim;
use crate::Unbreakable;
use crate::WireError;
use crate::check_saved_form::MAX_COUNT;
use crate::check_saved_form::NOT_NEGATIVE;
use crate::check_saved_form::OMINOUS_AMPLIFIER;
use crate::check_saved_form::POSITIVE;
use crate::check_saved_form::STACK_SIZE;
use crate::lore::MAX_LINES;
use crate::nbt_depth::LIST_TEMPLATE_LEVELS;
use crate::nbt_depth::SLOT_TEMPLATE_LEVELS;
use crate::nbt_depth::STACK_LEVEL;
use crate::saved::decode;
use crate::saved::is_air;
use crate::saved_sink::Field;
use crate::saved_sink::KeyedEntry;
use crate::saved_sink::Leaf;
use crate::saved_sink::MapKey;
use crate::saved_sink::NamedEntry;
use crate::saved_sink::SavedSink;
use crate::saved_sink::WireOnly;
use mcrs_minecraft_nbt::MAX_DEPTH;

pub(crate) const DISPLAY_DEFAULT: &str = "default";
pub(crate) const DISPLAY_HIDDEN: &str = "hidden";
pub(crate) const DISPLAY_OVERRIDE: &str = "override";

const DEFAULT_MINING_SPEED: f32 = 1.0;
const DEFAULT_DAMAGE_PER_BLOCK: i32 = 1;

#[derive(Clone, Copy, Debug)]
pub(crate) enum Fault {
    BadValue(BuiltinKey),
    OutOfRange(BuiltinKey),
    UnnamedId {
        component: BuiltinKey,
        table: NameTable,
    },
    TooDeep,
    WireValueOnly {
        component: BuiltinKey,
        component_type: ComponentType,
    },
}

impl Fault {
    pub(crate) fn into_saved(self) -> SavedError {
        match self {
            Self::BadValue(component) => SavedError::BadValue(component),
            Self::OutOfRange(component) => SavedError::OutOfRange(component),
            Self::UnnamedId { table, .. } => SavedError::UnnamedId(table),
            Self::TooDeep => SavedError::TooDeep,
            Self::WireValueOnly { component_type, .. } => SavedError::WireValueOnly(component_type),
        }
    }

    pub(crate) fn into_hash(self) -> HashError {
        match self {
            Self::BadValue(component) => HashError::BadValue(component),
            Self::OutOfRange(component) => HashError::OutOfRange(component),
            Self::UnnamedId { component, table } => HashError::UnknownEntry { component, table },
            Self::TooDeep => HashError::TooDeep,
            Self::WireValueOnly { component_type, .. } => {
                HashError::UndecodedComponent(component_type)
            }
        }
    }

    pub(crate) fn into_wire(self) -> WireError {
        match self {
            Self::BadValue(component) => WireError::BadValue(component),
            Self::OutOfRange(component) => WireError::OutOfRange(component),
            Self::UnnamedId { component, table } => WireError::UnknownEntry { component, table },
            Self::TooDeep => WireError::TooDeep,
            Self::WireValueOnly { component, .. } => WireError::BadValue(component),
        }
    }
}

pub(crate) fn saved_form<S: SavedSink, L: NameLookup>(
    value: &Builtin,
    names: &L,
    sink: &mut S,
) -> Result<S::Node, Fault> {
    saved_form_at(value, names, sink, STACK_LEVEL)
}

fn saved_form_at<S: SavedSink, L: NameLookup>(
    value: &Builtin,
    names: &L,
    sink: &mut S,
    depth: u32,
) -> Result<S::Node, Fault> {
    let walk = Walk {
        component: value.key(),
        names,
        depth,
    };
    match value {
        Builtin::ChargedProjectiles(ChargedProjectiles(projectiles)) => {
            if projectiles.len() > ChargedProjectiles::MAX_PROJECTILES {
                return Err(Fault::OutOfRange(walk.component));
            }
            sink.list(projectiles.iter(), |sink, stack| {
                walk.template(sink, stack, LIST_TEMPLATE_LEVELS)
            })
        }
        Builtin::BundleContents(BundleContents(stacks)) => sink
            .list(stacks.iter(), |sink, stack| {
                walk.template(sink, stack, LIST_TEMPLATE_LEVELS)
            }),
        Builtin::Container(container) => walk.container(sink, container),
        _ => flat_saved_form(&walk, value, sink),
    }
}

#[inline(never)]
fn flat_saved_form<S: SavedSink, L: NameLookup>(
    walk: &Walk<'_, L>,
    value: &Builtin,
    sink: &mut S,
) -> Result<S::Node, Fault> {
    match value {
        Builtin::MaxStackSize(MaxStackSize(size)) => walk.int(sink, *size, STACK_SIZE),
        Builtin::MaxDamage(MaxDamage(damage)) => walk.int(sink, *damage, POSITIVE),
        Builtin::Damage(Damage(damage)) => walk.int(sink, *damage, NOT_NEGATIVE),
        Builtin::RepairCost(RepairCost(cost)) => walk.int(sink, *cost, NOT_NEGATIVE),
        Builtin::MapId(MapId(id)) => Ok(sink.leaf(Leaf::Int(*id))),
        Builtin::MapDecorations(decorations) => walk.map_decorations(sink, decorations),
        Builtin::Profile(profile) => sink.profile(profile).ok_or(Fault::BadValue(walk.component)),
        Builtin::Unbreakable(Unbreakable) => Ok(sink.fields([])),
        Builtin::ItemModel(model) => Ok(sink.leaf(Leaf::Name(model.get()))),
        Builtin::Rarity(rarity) => Ok(sink.leaf(Leaf::String(rarity.name()))),
        Builtin::Enchantments(enchantments)
        | Builtin::StoredEnchantments(StoredEnchantments(enchantments)) => {
            walk.enchantments(sink, enchantments)
        }
        Builtin::PaintingVariant(PaintingVariant(PaintingVariantId(id))) => {
            walk.entry(sink, NameTable::PaintingVariant, *id)
        }
        Builtin::AttributeModifiers(modifiers) => walk.attribute_modifiers(sink, modifiers),
        Builtin::TooltipDisplay(display) => walk.tooltip_display(sink, display),
        Builtin::Food(food) => walk.food(sink, food),
        Builtin::Tool(tool) => walk.tool(sink, tool),
        Builtin::Equippable(equippable) => walk.equippable(sink, equippable),
        Builtin::PiercingWeapon(weapon) => walk.piercing_weapon(sink, weapon),
        Builtin::AttackAnimation(AttackAnimation(swing))
        | Builtin::InteractAnimation(InteractAnimation(swing)) => walk.swing_animation(sink, swing),
        Builtin::CustomName(CustomName(text)) | Builtin::ItemName(ItemName(text)) => {
            walk.text(sink, text)
        }
        Builtin::Lore(lore) => walk.lore(sink, lore),
        Builtin::DyedColor(DyedColor(color)) => Ok(sink.leaf(Leaf::Int(color.0))),
        Builtin::CustomModelData(data) => walk.custom_model_data(sink, data),
        Builtin::EnchantmentGlintOverride(EnchantmentGlintOverride(value)) => {
            Ok(sink.leaf(Leaf::Boolean(*value)))
        }
        Builtin::BlockState(state) => sink.keyed_map(state.properties(), |sink, property| {
            Ok(KeyedEntry {
                key: MapKey::String(&property.name),
                value: sink.leaf(Leaf::String(&property.value)),
            })
        }),
        Builtin::LodestoneTracker(tracker) => Ok(walk.lodestone_tracker(sink, tracker)),
        Builtin::Trim(trim) => walk.trim(sink, trim),
        Builtin::PotionContents(contents) => walk.potion_contents(sink, contents),
        Builtin::CustomData(CustomData(compound)) => sink
            .compound(compound)
            .ok_or(Fault::BadValue(walk.component)),
        Builtin::SuspiciousStewEffects(SuspiciousStewEffects(effects)) => {
            sink.list(effects.iter(), |sink, effect| {
                let id = walk.entry(sink, NameTable::MobEffect, effect.effect.0)?;
                let duration = (effect.duration != StewEffect::DEFAULT_DURATION)
                    .then(|| sink.leaf(Leaf::Int(effect.duration)));
                Ok(sink.fields([Field::optional("duration", duration), Field::new("id", id)]))
            })
        }
        Builtin::OminousBottleAmplifier(OminousBottleAmplifier(amplifier)) => {
            walk.int(sink, *amplifier, OMINOUS_AMPLIFIER)
        }
        Builtin::Instrument(Instrument(instrument)) => walk.instrument(sink, instrument),
        Builtin::ChargedProjectiles(_) | Builtin::BundleContents(_) | Builtin::Container(_) => {
            saved_form_at(value, walk.names, sink, walk.depth)
        }
        Builtin::FireworkExplosion(explosion) => walk.firework_explosion(sink, explosion),
        Builtin::Fireworks(fireworks) => walk.fireworks(sink, fireworks),
        Builtin::BannerPatterns(BannerPatterns(layers)) => {
            sink.list(layers.iter(), |sink, layer| walk.banner_layer(sink, layer))
        }
        Builtin::Bees(Bees(occupants)) => sink.list(occupants.iter(), |sink, occupant| {
            let entity_data = walk.entity_data(sink, &occupant.entity_data)?;
            Ok(sink.fields([
                Field::new("entity_data", entity_data),
                Field::new(
                    "ticks_in_hive",
                    sink.leaf(Leaf::Int(occupant.ticks_in_hive)),
                ),
                Field::new(
                    "min_ticks_in_hive",
                    sink.leaf(Leaf::Int(occupant.min_ticks_in_hive)),
                ),
            ]))
        }),
        Builtin::ContainerLoot(loot) => Ok(walk.container_loot(sink, loot)),
    }
}

pub(crate) fn is_use_duration(value: f32) -> bool {
    value.total_cmp(&0.0).is_ge() && value <= f32::MAX
}

pub(crate) fn is_instrument_range(value: f32) -> bool {
    value.total_cmp(&0.0).is_gt() && value <= f32::MAX
}

fn listed<S: SavedSink, T>(
    sink: &mut S,
    items: &[T],
    element: impl FnMut(&mut S, &T) -> Result<S::Node, Fault>,
) -> Result<Option<S::Node>, Fault> {
    if items.is_empty() {
        return Ok(None);
    }
    sink.list(items.iter(), element).map(Some)
}

struct Walk<'names, L> {
    component: BuiltinKey,
    names: &'names L,
    depth: u32,
}

impl<L: NameLookup> Walk<'_, L> {
    fn int<S: SavedSink>(
        &self,
        sink: &S,
        value: i32,
        range: RangeInclusive<i32>,
    ) -> Result<S::Node, Fault> {
        if range.contains(&value) {
            Ok(sink.leaf(Leaf::Int(value)))
        } else {
            Err(Fault::OutOfRange(self.component))
        }
    }

    fn name(&self, table: NameTable, id: u16) -> Result<ResourceLocation<'_>, Fault> {
        self.names.name(table, id).ok_or(Fault::UnnamedId {
            component: self.component,
            table,
        })
    }

    fn entry<S: SavedSink>(&self, sink: &S, table: NameTable, id: u16) -> Result<S::Node, Fault> {
        Ok(sink.leaf(Leaf::Name(self.name(table, id)?)))
    }

    fn text<S: SavedSink>(&self, sink: &mut S, text: &Text) -> Result<S::Node, Fault> {
        sink.text(text).ok_or(Fault::BadValue(self.component))
    }

    fn lore<S: SavedSink>(&self, sink: &mut S, lore: &Lore) -> Result<S::Node, Fault> {
        if lore.lines.len() > MAX_LINES {
            return Err(Fault::OutOfRange(self.component));
        }
        sink.list(lore.lines.iter(), |sink, line| self.text(sink, line))
    }

    fn enchantments<S: SavedSink>(
        &self,
        sink: &mut S,
        enchantments: &Enchantments,
    ) -> Result<S::Node, Fault> {
        sink.named_map(enchantments.entries().iter().map(|entry| {
            Ok(NamedEntry {
                name: self.name(NameTable::Enchantment, entry.enchantment.0)?,
                value: Leaf::Int(i32::from(entry.level.get())),
            })
        }))
    }

    fn attribute_modifiers<S: SavedSink>(
        &self,
        sink: &mut S,
        modifiers: &AttributeModifiers,
    ) -> Result<S::Node, Fault> {
        sink.list(modifiers.modifiers.iter(), |sink, modifier| {
            self.attribute_modifier(sink, modifier)
        })
    }

    fn attribute_modifier<S: SavedSink>(
        &self,
        sink: &mut S,
        modifier: &AttributeModifier,
    ) -> Result<S::Node, Fault> {
        let attribute = self.entry(sink, NameTable::Attribute, modifier.attribute.0)?;
        let display = match &modifier.display {
            ModifierDisplay::Default => None,
            ModifierDisplay::Hidden => {
                let kind = sink.leaf(Leaf::String(DISPLAY_HIDDEN));
                Some(sink.fields([Field::new("type", kind)]))
            }
            ModifierDisplay::Override(text) => {
                let kind = sink.leaf(Leaf::String(DISPLAY_OVERRIDE));
                let value = self.text(sink, text)?;
                Some(sink.fields([Field::new("type", kind), Field::new("value", value)]))
            }
        };
        let slot = (modifier.slot != SlotGroup::Any)
            .then(|| sink.leaf(Leaf::String(modifier.slot.name())));
        Ok(sink.fields([
            Field::new("type", attribute),
            Field::new("id", sink.leaf(Leaf::Name(modifier.id()))),
            Field::new("amount", sink.leaf(Leaf::Double(modifier.amount))),
            Field::new(
                "operation",
                sink.leaf(Leaf::String(modifier.operation.as_str())),
            ),
            Field::optional("slot", slot),
            Field::optional("display", display),
        ]))
    }

    fn tooltip_display<S: SavedSink>(
        &self,
        sink: &mut S,
        display: &TooltipDisplay,
    ) -> Result<S::Node, Fault> {
        let hide_tooltip = display.hide_tooltip.then(|| sink.leaf(Leaf::Boolean(true)));
        let hidden = display.hidden_components();
        let hidden_components = if hidden.is_empty() {
            None
        } else {
            Some(sink.list(hidden, |sink, component_type| {
                self.entry(sink, NameTable::ComponentType, component_type.0)
            })?)
        };
        Ok(sink.fields([
            Field::optional("hide_tooltip", hide_tooltip),
            Field::optional("hidden_components", hidden_components),
        ]))
    }

    fn food<S: SavedSink>(&self, sink: &S, food: &Food) -> Result<S::Node, Fault> {
        let nutrition = self.int(sink, food.nutrition, NOT_NEGATIVE)?;
        let can_always_eat = food.can_always_eat.then(|| sink.leaf(Leaf::Boolean(true)));
        Ok(sink.fields([
            Field::new("nutrition", nutrition),
            Field::new("saturation", sink.leaf(Leaf::Float(food.saturation))),
            Field::optional("can_always_eat", can_always_eat),
        ]))
    }

    fn tool<S: SavedSink>(&self, sink: &mut S, tool: &Tool) -> Result<S::Node, Fault> {
        let rules = sink.list(tool.rules.iter(), |sink, rule| self.tool_rule(sink, rule))?;
        let default_mining_speed = (tool.default_mining_speed.to_bits()
            != DEFAULT_MINING_SPEED.to_bits())
        .then(|| sink.leaf(Leaf::Float(tool.default_mining_speed)));
        let damage_per_block = if tool.damage_per_block == DEFAULT_DAMAGE_PER_BLOCK {
            None
        } else {
            Some(self.int(sink, tool.damage_per_block, NOT_NEGATIVE)?)
        };
        let can_destroy_blocks_in_creative =
            (!tool.can_destroy_blocks_in_creative).then(|| sink.leaf(Leaf::Boolean(false)));
        Ok(sink.fields([
            Field::new("rules", rules),
            Field::optional("default_mining_speed", default_mining_speed),
            Field::optional("damage_per_block", damage_per_block),
            Field::optional(
                "can_destroy_blocks_in_creative",
                can_destroy_blocks_in_creative,
            ),
        ]))
    }

    fn tool_rule<S: SavedSink>(&self, sink: &mut S, rule: &ToolRule) -> Result<S::Node, Fault> {
        let blocks = self.block_set(sink, &rule.blocks)?;
        let speed = rule.speed().map(|speed| sink.leaf(Leaf::Float(speed)));
        let correct_for_drops = rule
            .correct_for_drops
            .map(|correct_for_drops| sink.leaf(Leaf::Boolean(correct_for_drops)));
        Ok(sink.fields([
            Field::new("blocks", blocks),
            Field::optional("speed", speed),
            Field::optional("correct_for_drops", correct_for_drops),
        ]))
    }

    fn map_decorations<S: SavedSink>(
        &self,
        sink: &mut S,
        decorations: &MapDecorations,
    ) -> Result<S::Node, Fault> {
        sink.keyed_map(decorations.decorations(), |sink, decoration| {
            let kind = self.entry(sink, NameTable::MapDecorationType, decoration.kind.0)?;
            let value = sink.fields([
                Field::new("type", kind),
                Field::new("x", sink.leaf(Leaf::Double(decoration.x))),
                Field::new("z", sink.leaf(Leaf::Double(decoration.z))),
                Field::new("rotation", sink.leaf(Leaf::Float(decoration.rotation))),
            ]);
            Ok(KeyedEntry {
                key: MapKey::String(&decoration.key),
                value,
            })
        })
    }

    fn block_set<S: SavedSink>(&self, sink: &mut S, blocks: &BlockSet) -> Result<S::Node, Fault> {
        match blocks {
            BlockSet::Tag(block_tag) => {
                let name = self.name(NameTable::BlockTag, block_tag.0)?;
                Ok(sink.leaf(Leaf::TagName(name)))
            }
            BlockSet::Blocks(blocks) => {
                if let [block] = blocks.as_ref() {
                    return self.entry(sink, NameTable::Block, block.0);
                }
                sink.list(blocks.iter(), |sink, block| {
                    self.entry(sink, NameTable::Block, block.0)
                })
            }
        }
    }

    fn equippable<S: SavedSink>(
        &self,
        sink: &mut S,
        equippable: &Equippable,
    ) -> Result<S::Node, Fault> {
        let equip_sound = (equippable.equip_sound.as_ref())
            .map(|sound| self.sound_event(sink, sound))
            .transpose()?;
        let allowed_entities = (equippable.allowed_entities.as_ref())
            .map(|entity_types| self.entity_type_set(sink, entity_types))
            .transpose()?;
        let shearing_sound = (equippable.shearing_sound.as_ref())
            .map(|sound| self.sound_event(sink, sound))
            .transpose()?;
        let name = |name: ResourceLocation<'_>| sink.leaf(Leaf::Name(name));
        let flag =
            |value: bool, absent: bool| (value != absent).then(|| sink.leaf(Leaf::Boolean(value)));
        Ok(sink.fields([
            Field::new("slot", sink.leaf(Leaf::String(equippable.slot.name()))),
            Field::optional("equip_sound", equip_sound),
            Field::optional("asset_id", equippable.asset_id().map(name)),
            Field::optional("camera_overlay", equippable.camera_overlay().map(name)),
            Field::optional("allowed_entities", allowed_entities),
            Field::optional("dispensable", flag(equippable.dispensable, true)),
            Field::optional("swappable", flag(equippable.swappable, true)),
            Field::optional("damage_on_hurt", flag(equippable.damage_on_hurt, true)),
            Field::optional(
                "equip_on_interact",
                flag(equippable.equip_on_interact, false),
            ),
            Field::optional("can_be_sheared", flag(equippable.can_be_sheared, false)),
            Field::optional("shearing_sound", shearing_sound),
        ]))
    }

    fn sound_event<S: SavedSink>(&self, sink: &S, sound: &SoundEventRef) -> Result<S::Node, Fault> {
        match sound {
            SoundEventRef::Reference(id) => self.entry(sink, NameTable::SoundEvent, id.0),
            SoundEventRef::Inline(inline) => {
                let range = inline.range.map(|range| sink.leaf(Leaf::Float(range)));
                Ok(sink.fields([
                    Field::new("sound_id", sink.leaf(Leaf::Name(inline.sound_id()))),
                    Field::optional("range", range),
                ]))
            }
        }
    }

    fn custom_model_data<S: SavedSink>(
        &self,
        sink: &mut S,
        data: &CustomModelData,
    ) -> Result<S::Node, Fault> {
        let floats = listed(sink, &data.floats, |sink, value| {
            Ok(sink.leaf(Leaf::Float(*value)))
        })?;
        let flags = listed(sink, &data.flags, |sink, value| {
            Ok(sink.leaf(Leaf::Boolean(*value)))
        })?;
        let strings = listed(sink, &data.strings, |sink, value| {
            Ok(sink.leaf(Leaf::String(value)))
        })?;
        let colors = listed(sink, &data.colors, |sink, color| {
            Ok(sink.leaf(Leaf::Int(color.0)))
        })?;
        Ok(sink.fields([
            Field::optional("floats", floats),
            Field::optional("flags", flags),
            Field::optional("strings", strings),
            Field::optional("colors", colors),
        ]))
    }

    fn lodestone_tracker<S: SavedSink>(&self, sink: &S, tracker: &LodestoneTracker) -> S::Node {
        let target = tracker.target.as_ref().map(|target| {
            let pos = [target.pos.x, target.pos.y, target.pos.z];
            sink.fields([
                Field::new("dimension", sink.leaf(Leaf::Name(target.dimension()))),
                Field::new("pos", sink.leaf(Leaf::IntArray(&pos))),
            ])
        });
        let tracked = (!tracker.tracked).then(|| sink.leaf(Leaf::Boolean(false)));
        sink.fields([
            Field::optional("target", target),
            Field::optional("tracked", tracked),
        ])
    }

    fn trim<S: SavedSink>(&self, sink: &mut S, trim: &Trim) -> Result<S::Node, Fault> {
        let material = match &trim.material {
            RegistryValue::Reference(id) => self.entry(sink, NameTable::TrimMaterial, id.0)?,
            RegistryValue::Inline(material) => {
                let description = self.text(sink, &material.description)?;
                sink.fields([
                    Field::new("palette_id", sink.leaf(Leaf::Name(material.palette_id()))),
                    Field::new("description", description),
                ])
            }
        };
        let pattern = match &trim.pattern {
            RegistryValue::Reference(id) => self.entry(sink, NameTable::TrimPattern, id.0)?,
            RegistryValue::Inline(pattern) => {
                let description = self.text(sink, &pattern.description)?;
                sink.fields([
                    Field::new("asset_id", sink.leaf(Leaf::Name(pattern.asset_id()))),
                    Field::new("description", description),
                    Field::new("decal", sink.leaf(Leaf::Boolean(pattern.decal))),
                ])
            }
        };
        Ok(sink.fields([
            Field::new("material", material),
            Field::new("pattern", pattern),
        ]))
    }

    fn potion_contents<S: SavedSink>(
        &self,
        sink: &mut S,
        contents: &PotionContents,
    ) -> Result<S::Node, Fault> {
        let potion = (contents.potion)
            .map(|potion| self.entry(sink, NameTable::Potion, potion.0))
            .transpose()?;
        let custom_effects = listed(sink, &contents.custom_effects, |sink, effect| {
            self.effect_instance(sink, effect)
        })?;
        let custom_color = (contents.custom_color).map(|color| sink.leaf(Leaf::Int(color)));
        let custom_name =
            (contents.custom_name.as_deref()).map(|name| sink.leaf(Leaf::String(name)));
        Ok(sink.fields([
            Field::optional("potion", potion),
            Field::optional("custom_color", custom_color),
            Field::optional("custom_effects", custom_effects),
            Field::optional("custom_name", custom_name),
        ]))
    }

    fn effect_instance<S: SavedSink>(
        &self,
        sink: &S,
        effect: &EffectInstance,
    ) -> Result<S::Node, Fault> {
        let id = self.entry(sink, NameTable::MobEffect, effect.effect.0)?;
        let amplifier =
            (effect.amplifier != 0).then(|| sink.leaf(Leaf::Byte(effect.amplifier.cast_signed())));
        let duration = (effect.duration != 0).then(|| sink.leaf(Leaf::Int(effect.duration)));
        let ambient = effect.ambient.then(|| sink.leaf(Leaf::Boolean(true)));
        let show_particles = (!effect.show_particles).then(|| sink.leaf(Leaf::Boolean(false)));
        Ok(sink.fields([
            Field::new("id", id),
            Field::optional("amplifier", amplifier),
            Field::optional("duration", duration),
            Field::optional("ambient", ambient),
            Field::optional("show_particles", show_particles),
            Field::new("show_icon", sink.leaf(Leaf::Boolean(effect.show_icon))),
        ]))
    }

    fn instrument<S: SavedSink>(
        &self,
        sink: &mut S,
        instrument: &RegistryValue<InstrumentId, InlineInstrument>,
    ) -> Result<S::Node, Fault> {
        let inline = match instrument {
            RegistryValue::Reference(id) => {
                return self.entry(sink, NameTable::Instrument, id.0);
            }
            RegistryValue::Inline(inline) => inline,
        };
        if !is_use_duration(inline.use_duration)
            || !is_instrument_range(inline.range)
            || inline.durability_damage < 0
        {
            return Err(Fault::OutOfRange(self.component));
        }
        let description = self.text(sink, &inline.description)?;
        let sound_event = self.sound_event(sink, &inline.sound_event)?;
        let durability_damage =
            (inline.durability_damage != 0).then(|| sink.leaf(Leaf::Int(inline.durability_damage)));
        Ok(sink.fields([
            Field::new("sound_event", sound_event),
            Field::new("use_duration", sink.leaf(Leaf::Float(inline.use_duration))),
            Field::new("range", sink.leaf(Leaf::Float(inline.range))),
            Field::optional("durability_damage", durability_damage),
            Field::new("description", description),
        ]))
    }

    fn entity_type_set<S: SavedSink>(
        &self,
        sink: &mut S,
        entity_types: &EntityTypeSet,
    ) -> Result<S::Node, Fault> {
        match entity_types {
            EntityTypeSet::Tag(entity_tag) => {
                let name = self.name(NameTable::EntityTypeTag, entity_tag.0)?;
                Ok(sink.leaf(Leaf::TagName(name)))
            }
            EntityTypeSet::EntityTypes(entity_types) => {
                if let [entity_type] = entity_types.as_ref() {
                    return self.entry(sink, NameTable::EntityType, entity_type.0);
                }
                sink.list(entity_types.iter(), |sink, entity_type| {
                    self.entry(sink, NameTable::EntityType, entity_type.0)
                })
            }
        }
    }

    fn template<S: SavedSink>(
        &self,
        sink: &mut S,
        stack: &ItemStack,
        levels: u32,
    ) -> Result<S::Node, Fault> {
        let depth = self.depth.saturating_add(levels);
        if depth > MAX_DEPTH {
            return Err(Fault::TooDeep);
        }
        let item = self.name(NameTable::Item, stack.kind.0)?;
        if is_air(item) {
            return Err(Fault::BadValue(self.component));
        }
        let count = stack.count.get();
        if count > MAX_COUNT {
            return Err(Fault::OutOfRange(self.component));
        }
        saved_patch(
            stack,
            self.names,
            sink,
            depth,
            Fault::BadValue(self.component),
        )
        .map(|components| template_fields(sink, item, count, components))
    }

    fn container<S: SavedSink>(
        &self,
        sink: &mut S,
        container: &Container,
    ) -> Result<S::Node, Fault> {
        let slots = container.slots().iter().enumerate();
        let filled = slots.filter_map(|(slot, stack)| Some((slot, stack.as_ref()?)));
        sink.list(filled, |sink, (slot, stack)| {
            let slot = i32::try_from(slot).map_err(|_| Fault::OutOfRange(self.component))?;
            let item = self.template(sink, stack, SLOT_TEMPLATE_LEVELS)?;
            Ok(sink.fields([
                Field::new("slot", sink.leaf(Leaf::Int(slot))),
                Field::new("item", item),
            ]))
        })
    }

    fn container_loot<S: SavedSink>(&self, sink: &S, loot: &ContainerLoot) -> S::Node {
        let seed = (loot.seed != 0).then(|| sink.leaf(Leaf::Long(loot.seed)));
        sink.fields([
            Field::optional("seed", seed),
            Field::new(
                "loot_table",
                sink.leaf(Leaf::Name(loot.loot_table.as_location())),
            ),
        ])
    }

    fn entity_data<S: SavedSink>(&self, sink: &mut S, data: &EntityData) -> Result<S::Node, Fault> {
        let entity_type = self.name(NameTable::EntityType, data.entity_type.0)?;
        sink.entity_data(data.data(), entity_type)
            .ok_or(Fault::BadValue(self.component))
    }

    fn firework_explosion<S: SavedSink>(
        &self,
        sink: &mut S,
        explosion: &FireworkExplosion,
    ) -> Result<S::Node, Fault> {
        let colors = listed(sink, &explosion.colors, |sink, color| {
            Ok(sink.leaf(Leaf::Int(*color)))
        })?;
        let fade_colors = listed(sink, &explosion.fade_colors, |sink, color| {
            Ok(sink.leaf(Leaf::Int(*color)))
        })?;
        let has_trail = explosion.has_trail.then(|| sink.leaf(Leaf::Boolean(true)));
        let has_twinkle = explosion
            .has_twinkle
            .then(|| sink.leaf(Leaf::Boolean(true)));
        Ok(sink.fields([
            Field::new("shape", sink.leaf(Leaf::String(explosion.shape.name()))),
            Field::optional("colors", colors),
            Field::optional("fade_colors", fade_colors),
            Field::optional("has_trail", has_trail),
            Field::optional("has_twinkle", has_twinkle),
        ]))
    }

    fn fireworks<S: SavedSink>(
        &self,
        sink: &mut S,
        fireworks: &Fireworks,
    ) -> Result<S::Node, Fault> {
        if fireworks.explosions.len() > Fireworks::MAX_EXPLOSIONS {
            return Err(Fault::OutOfRange(self.component));
        }
        let flight_duration = (fireworks.flight_duration != 0)
            .then(|| sink.leaf(Leaf::Byte(fireworks.flight_duration.cast_signed())));
        let explosions = listed(sink, &fireworks.explosions, |sink, explosion| {
            self.firework_explosion(sink, explosion)
        })?;
        Ok(sink.fields([
            Field::optional("flight_duration", flight_duration),
            Field::optional("explosions", explosions),
        ]))
    }

    fn banner_layer<S: SavedSink>(
        &self,
        sink: &mut S,
        layer: &BannerLayer,
    ) -> Result<S::Node, Fault> {
        let pattern = match &layer.pattern {
            RegistryValue::Reference(id) => self.entry(sink, NameTable::BannerPattern, id.0)?,
            RegistryValue::Inline(pattern) => sink.fields([
                Field::new("asset_id", sink.leaf(Leaf::Name(pattern.asset_id()))),
                Field::new(
                    "translation_key",
                    sink.leaf(Leaf::String(&pattern.translation_key)),
                ),
            ]),
        };
        Ok(sink.fields([
            Field::new("pattern", pattern),
            Field::new("color", sink.leaf(Leaf::String(layer.color.name()))),
        ]))
    }

    fn piercing_weapon<S: SavedSink>(
        &self,
        sink: &mut S,
        weapon: &PiercingWeapon,
    ) -> Result<S::Node, Fault> {
        let sound = weapon
            .sound
            .as_ref()
            .map(|sound| self.sound_event(sink, sound))
            .transpose()?;
        let hit_sound = weapon
            .hit_sound
            .as_ref()
            .map(|sound| self.sound_event(sink, sound))
            .transpose()?;
        let flag =
            |value: bool, absent: bool| (value != absent).then(|| sink.leaf(Leaf::Boolean(value)));
        Ok(sink.fields([
            Field::optional("deals_knockback", flag(weapon.deals_knockback, true)),
            Field::optional("dismounts", flag(weapon.dismounts, false)),
            Field::optional("sound", sound),
            Field::optional("hit_sound", hit_sound),
        ]))
    }

    fn swing_animation<S: SavedSink>(
        &self,
        sink: &mut S,
        swing: &SwingAnimation,
    ) -> Result<S::Node, Fault> {
        if !NOT_NEGATIVE.contains(&swing.duration) {
            return Err(Fault::OutOfRange(self.component));
        }
        let kind = (swing.kind != SwingAnimation::DEFAULT_KIND)
            .then(|| sink.leaf(Leaf::String(swing.kind.name())));
        let duration = (swing.duration != SwingAnimation::DEFAULT_DURATION)
            .then(|| sink.leaf(Leaf::Int(swing.duration)));
        Ok(sink.fields([
            Field::optional("type", kind),
            Field::optional("duration", duration),
        ]))
    }
}

pub(crate) fn saved_patch<S: SavedSink, L: NameLookup>(
    stack: &ItemStack,
    names: &L,
    sink: &mut S,
    depth: u32,
    unsaved: Fault,
) -> Result<Option<S::Node>, Fault> {
    let mut entries = stack
        .patch()
        .iter()
        .filter(|entry| is_saved(entry, S::WIRE_ONLY))
        .peekable();
    if entries.peek().is_none() {
        return Ok(None);
    }
    sink.keyed_map(entries, |sink, entry| {
        patch_entry(names, sink, entry, depth, unsaved)
    })
    .map(Some)
}

fn patch_entry<'entry, S: SavedSink, L: NameLookup>(
    names: &L,
    sink: &mut S,
    entry: &'entry PatchEntry,
    depth: u32,
    unsaved: Fault,
) -> Result<KeyedEntry<'entry, S::Node>, Fault> {
    match entry {
        PatchEntry::Value(value) => {
            let key = MapKey::Name(value.key().location().ok_or(unsaved)?);
            saved_form_at(value, names, sink, depth).map(|value| KeyedEntry { key, value })
        }
        _ => flat_patch_entry(names, sink, entry, depth, unsaved),
    }
}

#[inline(never)]
fn flat_patch_entry<'entry, S: SavedSink, L: NameLookup>(
    names: &L,
    sink: &mut S,
    entry: &'entry PatchEntry,
    depth: u32,
    unsaved: Fault,
) -> Result<KeyedEntry<'entry, S::Node>, Fault> {
    Ok(match entry {
        PatchEntry::Value(_) => return patch_entry(names, sink, entry, depth, unsaved),
        PatchEntry::Removal(component) => KeyedEntry {
            key: MapKey::Removal(component.location().ok_or(unsaved)?),
            value: sink.fields([]),
        },
        PatchEntry::Undecoded(value) => {
            let tag = decode(&value.bytes).map_err(|_| Fault::TooDeep)?;
            KeyedEntry {
                key: MapKey::Name(value.name.as_location()),
                value: sink.tag(&tag).ok_or(unsaved)?,
            }
        }
        PatchEntry::UndecodedRemoval(name) => KeyedEntry {
            key: MapKey::Removal(name.as_location()),
            value: sink.fields([]),
        },
        PatchEntry::UndecodedWire(value) => {
            return Err(match unsaved {
                Fault::BadValue(component) => Fault::WireValueOnly {
                    component,
                    component_type: value.component(),
                },
                other => other,
            });
        }
        PatchEntry::Custom(_) => return Err(unsaved),
    })
}

#[inline(never)]
fn template_fields<S: SavedSink>(
    sink: &S,
    item: ResourceLocation<'_>,
    count: u8,
    components: Option<S::Node>,
) -> S::Node {
    let count = (count != 1).then(|| sink.leaf(Leaf::Int(i32::from(count))));
    sink.fields([
        Field::new("id", sink.leaf(Leaf::Name(item))),
        Field::optional("count", count),
        Field::optional("components", components),
    ])
}

fn is_saved(entry: &PatchEntry, wire_only: WireOnly) -> bool {
    match entry {
        PatchEntry::Custom(_) => false,
        PatchEntry::UndecodedWire(_) => wire_only == WireOnly::Refuse,
        PatchEntry::Value(_)
        | PatchEntry::Removal(_)
        | PatchEntry::Undecoded(_)
        | PatchEntry::UndecodedRemoval(_) => true,
    }
}
