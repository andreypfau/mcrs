use fixedbitset::FixedBitSet;
use mcrs_minecraft_item_key::item_keys::Item;
use mcrs_minecraft_item_stack::{ItemStack, NameLookup};
use mcrs_minecraft_item_table::ItemTable;
use mcrs_minecraft_registries::Registries;

use crate::baked_component_predicate::BakedComponentPredicate;
use crate::component_holder::ComponentHolder;
use crate::exact_component::ExactComponent;
use crate::id_bits::id_bits;
use crate::item_subject::ItemSubject;
use crate::predicate_issue::PredicateIssue;
use mcrs_minecraft_item::component::common::MinMaxBounds;
use mcrs_minecraft_item::component::predicate::ItemPredicate;

#[derive(Clone, Debug, PartialEq)]
pub struct BakedItemPredicate {
    items: Option<FixedBitSet>,
    count: MinMaxBounds<i32>,
    components: Box<[ExactComponent]>,
    predicates: Box<[BakedComponentPredicate]>,
}

impl BakedItemPredicate {
    pub fn bake<L: NameLookup>(
        predicate: &ItemPredicate,
        registries: &Registries,
        names: &L,
    ) -> Result<Self, PredicateIssue> {
        let items = predicate
            .items
            .as_ref()
            .map(|set| id_bits(set, registries, PredicateIssue::UnloadedItemTags))
            .transpose()?;
        let components: Box<[ExactComponent]> = predicate
            .matchers
            .components
            .0
            .iter()
            .map(|value| ExactComponent::bake(value, registries, names))
            .collect::<Result<_, _>>()?;
        let predicates: Box<[BakedComponentPredicate]> = predicate
            .matchers
            .predicates
            .0
            .iter()
            .map(|entry| BakedComponentPredicate::bake(entry, registries))
            .collect::<Result<_, _>>()?;
        Ok(Self {
            items,
            count: predicate.count,
            components,
            predicates,
        })
    }

    #[must_use]
    pub fn matches(&self, subject: ItemSubject<'_>, table: &ItemTable) -> bool {
        match subject {
            ItemSubject::Stack(stack) => {
                self.matches_count(stack, i32::from(stack.count.get()), table)
            }
            ItemSubject::Empty => self.test(Item::Air.id().number(), 0, ComponentHolder::Empty),
        }
    }

    #[must_use]
    pub fn matches_count(&self, stack: &ItemStack, count: i32, table: &ItemTable) -> bool {
        self.test(stack.kind.0, count, ComponentHolder::Stack { stack, table })
    }

    fn test(&self, item: u16, count: i32, holder: ComponentHolder<'_>) -> bool {
        self.items
            .as_ref()
            .is_none_or(|bits| bits.contains(usize::from(item)))
            && self.count.matches(count)
            && self
                .components
                .iter()
                .all(|component| component.matches(holder))
            && self
                .predicates
                .iter()
                .all(|predicate| predicate.matches(holder))
    }
}
