use core::ops::RangeInclusive;

use mcrs_minecraft_resource::ResourceLocation;
use mcrs_minecraft_text::Text;

use crate::AttributeModifier;
use crate::AttributeModifiers;
use crate::BlockSet;
use crate::Builtin;
use crate::BuiltinKey;
use crate::CustomName;
use crate::Damage;
use crate::Enchantments;
use crate::Food;
use crate::HashError;
use crate::ItemName;
use crate::Lore;
use crate::MaxDamage;
use crate::MaxStackSize;
use crate::ModifierDisplay;
use crate::NameLookup;
use crate::NameTable;
use crate::RepairCost;
use crate::SavedError;
use crate::SlotGroup;
use crate::Tool;
use crate::ToolRule;
use crate::TooltipDisplay;
use crate::Unbreakable;
use crate::check_saved_form::NOT_NEGATIVE;
use crate::check_saved_form::POSITIVE;
use crate::check_saved_form::STACK_SIZE;
use crate::lore::MAX_LINES;
use crate::saved_sink::Field;
use crate::saved_sink::Leaf;
use crate::saved_sink::NamedEntry;
use crate::saved_sink::SavedSink;

pub(crate) const DISPLAY_DEFAULT: &str = "default";
pub(crate) const DISPLAY_HIDDEN: &str = "hidden";
pub(crate) const DISPLAY_OVERRIDE: &str = "override";

const DEFAULT_MINING_SPEED: f32 = 1.0;
const DEFAULT_DAMAGE_PER_BLOCK: i32 = 1;

/// Why a component value has no saved form.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Fault {
    BadValue(BuiltinKey),
    OutOfRange(BuiltinKey),
    UnnamedId {
        component: BuiltinKey,
        table: NameTable,
    },
}

impl Fault {
    pub(crate) fn into_saved(self) -> SavedError {
        match self {
            Self::BadValue(component) => SavedError::BadValue(component),
            Self::OutOfRange(component) => SavedError::OutOfRange(component),
            Self::UnnamedId { table, .. } => SavedError::UnnamedId(table),
        }
    }

    pub(crate) fn into_hash(self) -> HashError {
        match self {
            Self::BadValue(component) => HashError::BadValue(component),
            Self::OutOfRange(component) => HashError::OutOfRange(component),
            Self::UnnamedId { component, table } => HashError::UnknownEntry { component, table },
        }
    }
}

/// Emits the saved form of a component to `sink`: each default omitted and each registry entry as
/// its identifier.
pub(crate) fn saved_form<S: SavedSink, L: NameLookup>(
    value: &Builtin,
    names: &L,
    sink: &mut S,
) -> Result<S::Node, Fault> {
    let walk = Walk {
        component: value.key(),
        names,
    };
    match value {
        Builtin::MaxStackSize(MaxStackSize(size)) => walk.int(sink, *size, STACK_SIZE),
        Builtin::MaxDamage(MaxDamage(damage)) => walk.int(sink, *damage, POSITIVE),
        Builtin::Damage(Damage(damage)) => walk.int(sink, *damage, NOT_NEGATIVE),
        Builtin::RepairCost(RepairCost(cost)) => walk.int(sink, *cost, NOT_NEGATIVE),
        Builtin::Unbreakable(Unbreakable) => Ok(sink.fields([])),
        Builtin::ItemModel(model) => Ok(sink.leaf(Leaf::Name(model.get()))),
        Builtin::Rarity(rarity) => Ok(sink.leaf(Leaf::String(rarity.name()))),
        Builtin::Enchantments(enchantments) => walk.enchantments(sink, enchantments),
        Builtin::AttributeModifiers(modifiers) => walk.attribute_modifiers(sink, modifiers),
        Builtin::TooltipDisplay(display) => walk.tooltip_display(sink, display),
        Builtin::Food(food) => walk.food(sink, food),
        Builtin::Tool(tool) => walk.tool(sink, tool),
        Builtin::CustomName(CustomName(text)) | Builtin::ItemName(ItemName(text)) => {
            walk.text(sink, text)
        }
        Builtin::Lore(lore) => walk.lore(sink, lore),
    }
}

struct Walk<'names, L> {
    component: BuiltinKey,
    names: &'names L,
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
                sink.leaf(Leaf::String(modifier.operation.name())),
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
}
