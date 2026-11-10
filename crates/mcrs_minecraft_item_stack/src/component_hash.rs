use core::ops::RangeInclusive;

use mcrs_minecraft_resource::ResourceLocation;

use crate::AttributeModifiers;
use crate::BlockSet;
use crate::Builtin;
use crate::BuiltinKey;
use crate::Damage;
use crate::Enchantments;
use crate::Food;
use crate::HashError;
use crate::HashScratch;
use crate::MaxDamage;
use crate::MaxStackSize;
use crate::ModifierDisplay;
use crate::NameLookup;
use crate::NameTable;
use crate::RepairCost;
use crate::SlotGroup;
use crate::Tool;
use crate::ToolRule;
use crate::TooltipDisplay;
use crate::Unbreakable;
use crate::field_map::FieldMap;
use crate::node_hash::ListHash;
use crate::node_hash::MapEntry;
use crate::node_hash::NodeHash;

const NOT_NEGATIVE: RangeInclusive<i32> = 0..=i32::MAX;
const DISPLAY_HIDDEN: &str = "hidden";
const DEFAULT_MINING_SPEED: f32 = 1.0;
const DEFAULT_DAMAGE_PER_BLOCK: i32 = 1;

/// The hash of the saved form of a component: the tree that the saved writer writes, with each
/// default omitted and each registry entry as its identifier.
pub(crate) fn component_hash<L: NameLookup>(
    value: &Builtin,
    names: &L,
    scratch: &mut HashScratch,
) -> Result<NodeHash, HashError> {
    let tree = Tree {
        component: value.key(),
        names,
    };
    match value {
        Builtin::MaxStackSize(MaxStackSize(size)) => tree.int(*size, 1..=99),
        Builtin::MaxDamage(MaxDamage(damage)) => tree.int(*damage, 1..=i32::MAX),
        Builtin::Damage(Damage(damage)) => tree.int(*damage, NOT_NEGATIVE),
        Builtin::RepairCost(RepairCost(cost)) => tree.int(*cost, NOT_NEGATIVE),
        Builtin::Unbreakable(Unbreakable) => Ok(FieldMap::new().finish()),
        Builtin::ItemModel(model) => Ok(identifier(model.get())),
        Builtin::Rarity(rarity) => Ok(NodeHash::string(rarity.name())),
        Builtin::Enchantments(enchantments) => tree.enchantments(enchantments, scratch),
        Builtin::AttributeModifiers(modifiers) => tree.attribute_modifiers(modifiers),
        Builtin::TooltipDisplay(display) => tree.tooltip_display(display),
        Builtin::Food(food) => tree.food(food),
        Builtin::Tool(tool) => tree.tool(tool),
        Builtin::CustomName(_) | Builtin::ItemName(_) | Builtin::Lore(_) => {
            Err(HashError::TextNotSupported(tree.component))
        }
    }
}

fn identifier(location: ResourceLocation<'_>) -> NodeHash {
    NodeHash::string_of_parts(&[location.namespace(), ":", location.path()])
}

struct Tree<'names, L> {
    component: BuiltinKey,
    names: &'names L,
}

impl<L: NameLookup> Tree<'_, L> {
    fn int(&self, value: i32, range: RangeInclusive<i32>) -> Result<NodeHash, HashError> {
        if range.contains(&value) {
            Ok(NodeHash::int(value))
        } else {
            Err(HashError::OutOfRange(self.component))
        }
    }

    fn name(&self, table: NameTable, id: u16) -> Result<ResourceLocation<'_>, HashError> {
        self.names.name(table, id).ok_or(HashError::UnknownEntry {
            component: self.component,
            table,
        })
    }

    fn entry(&self, table: NameTable, id: u16) -> Result<NodeHash, HashError> {
        Ok(identifier(self.name(table, id)?))
    }

    fn enchantments(
        &self,
        enchantments: &Enchantments,
        scratch: &mut HashScratch,
    ) -> Result<NodeHash, HashError> {
        scratch.entries.clear();
        for entry in enchantments.entries() {
            scratch.entries.push(MapEntry {
                key: self.entry(NameTable::Enchantment, entry.enchantment.0)?,
                value: NodeHash::int(i32::from(entry.level.get())),
            });
        }
        Ok(NodeHash::map(&mut scratch.entries))
    }

    fn attribute_modifiers(&self, modifiers: &AttributeModifiers) -> Result<NodeHash, HashError> {
        let mut list = ListHash::new();
        for modifier in modifiers.modifiers.iter() {
            let mut fields = FieldMap::new();
            fields.put(
                "type",
                self.entry(NameTable::Attribute, modifier.attribute.0)?,
            );
            fields.put("id", identifier(modifier.id()));
            fields.put("amount", NodeHash::double(modifier.amount));
            fields.put("operation", NodeHash::string(modifier.operation.name()));
            if modifier.slot != SlotGroup::Any {
                fields.put("slot", NodeHash::string(modifier.slot.name()));
            }
            match modifier.display {
                ModifierDisplay::Default => {}
                ModifierDisplay::Hidden => {
                    let mut display = FieldMap::new();
                    display.put("type", NodeHash::string(DISPLAY_HIDDEN));
                    fields.put("display", display.finish());
                }
                ModifierDisplay::Override(_) => {
                    return Err(HashError::TextNotSupported(self.component));
                }
            }
            list = list.push(fields.finish());
        }
        Ok(list.finish())
    }

    fn tooltip_display(&self, display: &TooltipDisplay) -> Result<NodeHash, HashError> {
        let mut fields = FieldMap::new();
        if display.hide_tooltip {
            fields.put("hide_tooltip", NodeHash::boolean(true));
        }
        let hidden = display.hidden_components();
        if !hidden.is_empty() {
            let mut list = ListHash::new();
            for component_type in hidden {
                list = list.push(self.entry(NameTable::ComponentType, component_type.0)?);
            }
            fields.put("hidden_components", list.finish());
        }
        Ok(fields.finish())
    }

    fn food(&self, food: &Food) -> Result<NodeHash, HashError> {
        let mut fields = FieldMap::new();
        fields.put("nutrition", self.int(food.nutrition, NOT_NEGATIVE)?);
        fields.put("saturation", NodeHash::float(food.saturation));
        if food.can_always_eat {
            fields.put("can_always_eat", NodeHash::boolean(true));
        }
        Ok(fields.finish())
    }

    fn tool(&self, tool: &Tool) -> Result<NodeHash, HashError> {
        let mut rules = ListHash::new();
        for rule in tool.rules.iter() {
            rules = rules.push(self.tool_rule(rule)?);
        }
        let mut fields = FieldMap::new();
        fields.put("rules", rules.finish());
        if tool.default_mining_speed.to_bits() != DEFAULT_MINING_SPEED.to_bits() {
            fields.put(
                "default_mining_speed",
                NodeHash::float(tool.default_mining_speed),
            );
        }
        if tool.damage_per_block != DEFAULT_DAMAGE_PER_BLOCK {
            fields.put(
                "damage_per_block",
                self.int(tool.damage_per_block, NOT_NEGATIVE)?,
            );
        }
        if !tool.can_destroy_blocks_in_creative {
            fields.put("can_destroy_blocks_in_creative", NodeHash::boolean(false));
        }
        Ok(fields.finish())
    }

    fn tool_rule(&self, rule: &ToolRule) -> Result<NodeHash, HashError> {
        let mut fields = FieldMap::new();
        fields.put("blocks", self.block_set(&rule.blocks)?);
        if let Some(speed) = rule.speed() {
            fields.put("speed", NodeHash::float(speed));
        }
        if let Some(correct_for_drops) = rule.correct_for_drops {
            fields.put("correct_for_drops", NodeHash::boolean(correct_for_drops));
        }
        Ok(fields.finish())
    }

    fn block_set(&self, blocks: &BlockSet) -> Result<NodeHash, HashError> {
        match blocks {
            BlockSet::Tag(block_tag) => {
                let name = self.name(NameTable::BlockTag, block_tag.0)?;
                Ok(NodeHash::string_of_parts(&[
                    "#",
                    name.namespace(),
                    ":",
                    name.path(),
                ]))
            }
            BlockSet::Blocks(blocks) => {
                if let [block] = blocks.as_ref() {
                    return self.entry(NameTable::Block, block.0);
                }
                let mut list = ListHash::new();
                for block in blocks.iter() {
                    list = list.push(self.entry(NameTable::Block, block.0)?);
                }
                Ok(list.finish())
            }
        }
    }
}
