use fixedbitset::FixedBitSet;
use mcrs_minecraft_item_key::item_keys::Item;
use mcrs_minecraft_item_stack::NameLookup;
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

/// An item predicate in its matching form: each entry set as a bit per id, and each exact
/// component as a component of the value-form stack.
#[derive(Clone, Debug, PartialEq)]
pub struct BakedItemPredicate {
    items: Option<FixedBitSet>,
    count: MinMaxBounds<i32>,
    components: Box<[ExactComponent]>,
    predicates: Box<[BakedComponentPredicate]>,
}

impl BakedItemPredicate {
    /// Bakes `predicate` with the registries that resolved it. `names` gives the numeric ids that
    /// the components of the value-form stack hold, as the stack codec reads them.
    ///
    /// A match compares the item, enchantment and attribute ids of a stack with the ids of
    /// `registries`, so the stacks and the item table must come from the same registry set.
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

    /// True when `subject` passes each test of the predicate. `table` gives the prototype
    /// components of the item of the stack. A match allocates nothing.
    #[must_use]
    pub fn matches(&self, subject: ItemSubject<'_>, table: &ItemTable) -> bool {
        let (item, count, holder) = match subject {
            ItemSubject::Stack(stack) => (
                stack.kind.0,
                i32::from(stack.count.get()),
                ComponentHolder::Stack { stack, table },
            ),
            ItemSubject::Empty => (Item::Air.id().number(), 0, ComponentHolder::Empty),
        };
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
