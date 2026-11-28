use mcrs_minecraft_item_stack::AttributeModifiers;
use mcrs_minecraft_item_stack::Damage;
use mcrs_minecraft_item_stack::Enchantments;
use mcrs_minecraft_item_stack::MaxDamage;
use mcrs_minecraft_registries::Registries;
use mcrs_minecraft_value_provider::bounds::MinMaxBounds;

use crate::baked_collection::BakedCollection;
use crate::baked_enchantment_predicate::BakedEnchantmentPredicate;
use crate::baked_modifier_predicate::BakedModifierPredicate;
use crate::component_holder::ComponentHolder;
use crate::predicate_issue::PredicateIssue;
use mcrs_minecraft_item::component::predicate::ComponentPredicate;
use mcrs_minecraft_item::component::predicate::ComponentPredicateEntry;
use mcrs_minecraft_item::component::predicate::DamagePredicate;
use mcrs_minecraft_item::data_component_type_keys::DataComponentType;

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum BakedComponentPredicate {
    Present(DataComponentType),
    Damage(DamagePredicate),
    Enchantments(Box<[BakedEnchantmentPredicate]>),
    AttributeModifiers(Option<BakedCollection<BakedModifierPredicate>>),
    Always,
    Never,
}

impl BakedComponentPredicate {
    pub(crate) fn bake(
        entry: &ComponentPredicateEntry,
        registries: &Registries,
    ) -> Result<Self, PredicateIssue> {
        let predicate = match entry {
            ComponentPredicateEntry::AnyValue(kind) => return Ok(Self::Present(*kind)),
            ComponentPredicateEntry::Typed(predicate) => predicate,
        };
        Ok(match predicate {
            ComponentPredicate::Damage(damage) => Self::Damage(damage.clone()),
            ComponentPredicate::Enchantments(list) => Self::Enchantments(
                list.0
                    .iter()
                    .map(|element| BakedEnchantmentPredicate::bake(element, registries))
                    .collect::<Result<_, _>>()?,
            ),
            ComponentPredicate::AttributeModifiers(predicate) => Self::AttributeModifiers(
                predicate
                    .modifiers
                    .as_ref()
                    .map(|modifiers| {
                        BakedCollection::bake(modifiers, |element| {
                            BakedModifierPredicate::bake(element, registries)
                        })
                    })
                    .transpose()?,
            ),
            ComponentPredicate::CustomData(expected) if expected.0.is_empty() => Self::Always,
            ComponentPredicate::CustomData(_)
            | ComponentPredicate::StoredEnchantments(_)
            | ComponentPredicate::PotionContents(_)
            | ComponentPredicate::Container(_)
            | ComponentPredicate::BundleContents(_)
            | ComponentPredicate::FireworkExplosion(_)
            | ComponentPredicate::Fireworks(_)
            | ComponentPredicate::WritableBookContent(_)
            | ComponentPredicate::WrittenBookContent(_)
            | ComponentPredicate::Trim(_)
            | ComponentPredicate::JukeboxPlayable(_)
            | ComponentPredicate::VillagerVariant(_) => Self::Never,
        })
    }

    pub(crate) fn matches(&self, holder: ComponentHolder<'_>) -> bool {
        match self {
            Self::Present(kind) => holder.has(*kind),
            Self::Damage(predicate) => holder.effective::<Damage>().is_some_and(|damage| {
                let max = holder.effective::<MaxDamage>().map_or(0, |max| max.0);
                let durability = i64::from(max) - i64::from(damage.0);
                let bounds = MinMaxBounds {
                    min: predicate.durability.min.map(i64::from),
                    max: predicate.durability.max.map(i64::from),
                };
                bounds.matches(durability) && predicate.damage.matches(damage.0)
            }),
            Self::Enchantments(elements) => holder
                .effective::<Enchantments>()
                .is_some_and(|component| elements.iter().all(|element| element.matches(component))),
            Self::AttributeModifiers(modifiers) => holder
                .effective::<AttributeModifiers>()
                .is_some_and(|component| {
                    modifiers.as_ref().is_none_or(|collection| {
                        collection.matches(&component.modifiers, BakedModifierPredicate::matches)
                    })
                }),
            Self::Always => true,
            Self::Never => false,
        }
    }
}
