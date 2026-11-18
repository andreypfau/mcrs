use mcrs_minecraft_item_stack::AttributeModifiers;
use mcrs_minecraft_item_stack::BannerPatterns;
use mcrs_minecraft_item_stack::Bees;
use mcrs_minecraft_item_stack::BlockState;
use mcrs_minecraft_item_stack::Builtin;
use mcrs_minecraft_item_stack::BundleContents;
use mcrs_minecraft_item_stack::ChargedProjectiles;
use mcrs_minecraft_item_stack::Container;
use mcrs_minecraft_item_stack::ContainerLoot;
use mcrs_minecraft_item_stack::CustomData;
use mcrs_minecraft_item_stack::CustomModelData;
use mcrs_minecraft_item_stack::CustomName;
use mcrs_minecraft_item_stack::Damage;
use mcrs_minecraft_item_stack::DyedColor;
use mcrs_minecraft_item_stack::EnchantmentGlintOverride;
use mcrs_minecraft_item_stack::Enchantments;
use mcrs_minecraft_item_stack::FireworkExplosion;
use mcrs_minecraft_item_stack::Fireworks;
use mcrs_minecraft_item_stack::Food;
use mcrs_minecraft_item_stack::Instrument;
use mcrs_minecraft_item_stack::ItemComponent;
use mcrs_minecraft_item_stack::ItemModel;
use mcrs_minecraft_item_stack::ItemName;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_item_stack::LodestoneTracker;
use mcrs_minecraft_item_stack::Lore;
use mcrs_minecraft_item_stack::MapDecorations;
use mcrs_minecraft_item_stack::MapId;
use mcrs_minecraft_item_stack::MaxDamage;
use mcrs_minecraft_item_stack::MaxStackSize;
use mcrs_minecraft_item_stack::OminousBottleAmplifier;
use mcrs_minecraft_item_stack::PaintingVariant;
use mcrs_minecraft_item_stack::PotionContents;
use mcrs_minecraft_item_stack::Profile;
use mcrs_minecraft_item_stack::Rarity;
use mcrs_minecraft_item_stack::RepairCost;
use mcrs_minecraft_item_stack::StoredEnchantments;
use mcrs_minecraft_item_stack::SuspiciousStewEffects;
use mcrs_minecraft_item_stack::Tool;
use mcrs_minecraft_item_stack::TooltipDisplay;
use mcrs_minecraft_item_stack::Trim;
use mcrs_minecraft_item_stack::Unbreakable;
use mcrs_minecraft_item_table::ItemTable;

use mcrs_minecraft_item::data_component_type_keys::DataComponentType;

#[derive(Clone, Copy, Debug)]
pub(crate) enum ComponentHolder<'a> {
    Empty,
    Stack {
        stack: &'a ItemStack,
        table: &'a ItemTable,
    },
}

impl<'a> ComponentHolder<'a> {
    pub(crate) fn effective<T: ItemComponent + 'static>(self) -> Option<&'a T> {
        match self {
            Self::Empty => None,
            Self::Stack { stack, table } => table.effective(stack),
        }
    }

    pub(crate) fn has(self, kind: DataComponentType) -> bool {
        match kind {
            DataComponentType::MaxStackSize => self.effective::<MaxStackSize>().is_some(),
            DataComponentType::MaxDamage => self.effective::<MaxDamage>().is_some(),
            DataComponentType::Damage => self.effective::<Damage>().is_some(),
            DataComponentType::Unbreakable => self.effective::<Unbreakable>().is_some(),
            DataComponentType::CustomName => self.effective::<CustomName>().is_some(),
            DataComponentType::ItemName => self.effective::<ItemName>().is_some(),
            DataComponentType::ItemModel => self.effective::<ItemModel>().is_some(),
            DataComponentType::Lore => self.effective::<Lore>().is_some(),
            DataComponentType::Rarity => self.effective::<Rarity>().is_some(),
            DataComponentType::Enchantments => self.effective::<Enchantments>().is_some(),
            DataComponentType::AttributeModifiers => {
                self.effective::<AttributeModifiers>().is_some()
            }
            DataComponentType::TooltipDisplay => self.effective::<TooltipDisplay>().is_some(),
            DataComponentType::RepairCost => self.effective::<RepairCost>().is_some(),
            DataComponentType::Food => self.effective::<Food>().is_some(),
            DataComponentType::Tool => self.effective::<Tool>().is_some(),
            DataComponentType::MapId => self.effective::<MapId>().is_some(),
            DataComponentType::MapDecorations => self.effective::<MapDecorations>().is_some(),
            DataComponentType::Profile => self.effective::<Profile>().is_some(),
            DataComponentType::DyedColor => self.effective::<DyedColor>().is_some(),
            DataComponentType::CustomModelData => self.effective::<CustomModelData>().is_some(),
            DataComponentType::EnchantmentGlintOverride => {
                self.effective::<EnchantmentGlintOverride>().is_some()
            }
            DataComponentType::BlockState => self.effective::<BlockState>().is_some(),
            DataComponentType::LodestoneTracker => self.effective::<LodestoneTracker>().is_some(),
            DataComponentType::Trim => self.effective::<Trim>().is_some(),
            DataComponentType::PotionContents => self.effective::<PotionContents>().is_some(),
            DataComponentType::CustomData => self.effective::<CustomData>().is_some(),
            DataComponentType::SuspiciousStewEffects => {
                self.effective::<SuspiciousStewEffects>().is_some()
            }
            DataComponentType::OminousBottleAmplifier => {
                self.effective::<OminousBottleAmplifier>().is_some()
            }
            DataComponentType::Instrument => self.effective::<Instrument>().is_some(),
            DataComponentType::BannerPatterns => self.effective::<BannerPatterns>().is_some(),
            DataComponentType::Bees => self.effective::<Bees>().is_some(),
            DataComponentType::BundleContents => self.effective::<BundleContents>().is_some(),
            DataComponentType::ChargedProjectiles => {
                self.effective::<ChargedProjectiles>().is_some()
            }
            DataComponentType::Container => self.effective::<Container>().is_some(),
            DataComponentType::ContainerLoot => self.effective::<ContainerLoot>().is_some(),
            DataComponentType::FireworkExplosion => self.effective::<FireworkExplosion>().is_some(),
            DataComponentType::Fireworks => self.effective::<Fireworks>().is_some(),
            DataComponentType::StoredEnchantments => {
                self.effective::<StoredEnchantments>().is_some()
            }
            DataComponentType::PaintingVariant => self.effective::<PaintingVariant>().is_some(),
            _ => false,
        }
    }

    pub(crate) fn has_equal(self, expected: &Builtin) -> bool {
        match expected {
            Builtin::MaxStackSize(value) => self.effective() == Some(value),
            Builtin::MaxDamage(value) => self.effective() == Some(value),
            Builtin::Damage(value) => self.effective() == Some(value),
            Builtin::Unbreakable(value) => self.effective() == Some(value),
            Builtin::CustomName(value) => self.effective() == Some(value),
            Builtin::ItemName(value) => self.effective() == Some(value),
            Builtin::ItemModel(value) => self.effective() == Some(value),
            Builtin::Lore(value) => self.effective() == Some(value),
            Builtin::Rarity(value) => self.effective() == Some(value),
            Builtin::Enchantments(value) => self.effective() == Some(value),
            Builtin::AttributeModifiers(value) => self.effective() == Some(value),
            Builtin::TooltipDisplay(value) => self.effective() == Some(value),
            Builtin::RepairCost(value) => self.effective() == Some(value),
            Builtin::Food(value) => self.effective() == Some(value),
            Builtin::Tool(value) => self.effective() == Some(value),
            Builtin::MapId(value) => self.effective() == Some(value),
            Builtin::MapDecorations(value) => self.effective() == Some(value),
            Builtin::Profile(value) => self.effective() == Some(value),
            Builtin::DyedColor(value) => self.effective() == Some(value),
            Builtin::CustomModelData(value) => self.effective() == Some(value),
            Builtin::EnchantmentGlintOverride(value) => self.effective() == Some(value),
            Builtin::BlockState(value) => self.effective() == Some(value),
            Builtin::LodestoneTracker(value) => self.effective() == Some(value),
            Builtin::Trim(value) => self.effective() == Some(value),
            Builtin::PotionContents(value) => self.effective() == Some(value),
            Builtin::CustomData(value) => self.effective() == Some(value),
            Builtin::SuspiciousStewEffects(value) => self.effective() == Some(value),
            Builtin::OminousBottleAmplifier(value) => self.effective() == Some(value),
            Builtin::Instrument(value) => self.effective() == Some(value),
            Builtin::BannerPatterns(value) => self.effective() == Some(value),
            Builtin::Bees(value) => self.effective() == Some(value),
            Builtin::BundleContents(value) => self.effective() == Some(value),
            Builtin::ChargedProjectiles(value) => self.effective() == Some(value),
            Builtin::Container(value) => self.effective() == Some(value),
            Builtin::ContainerLoot(value) => self.effective() == Some(value),
            Builtin::FireworkExplosion(value) => self.effective() == Some(value),
            Builtin::Fireworks(value) => self.effective() == Some(value),
            Builtin::StoredEnchantments(value) => self.effective() == Some(value),
            Builtin::PaintingVariant(value) => self.effective() == Some(value),
            _ => false,
        }
    }
}
