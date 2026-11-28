use mcrs_minecraft_item_stack::Builtin;
use mcrs_minecraft_item_stack::NameLookup;
use mcrs_minecraft_item_stack::read_saved_component;
use mcrs_minecraft_nbt::to_nbt_tag;
use mcrs_minecraft_registries::Registries;
use mcrs_minecraft_registries::Resolve;

use crate::component_holder::ComponentHolder;
use crate::predicate_issue::PredicateIssue;
use mcrs_minecraft_item::ItemComponentValue;

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum ExactComponent {
    Equals(Builtin),
    Unmodeled,
}

impl ExactComponent {
    pub(crate) fn bake<L: NameLookup>(
        value: &ItemComponentValue,
        registries: &Registries,
        names: &L,
    ) -> Result<Self, PredicateIssue> {
        let kind = value.kind();
        if !kind.is_persistent() {
            return Ok(Self::Unmodeled);
        }
        let named = value
            .name(registries)
            .map_err(|_| PredicateIssue::UnnamedEntry)?;
        let saved = to_nbt_tag(&named).map_err(|_| PredicateIssue::UnreadableComponent)?;
        match read_saved_component(kind.location(), &saved, names) {
            Ok(Some(component)) => Ok(Self::Equals(component)),
            Ok(None) => Ok(Self::Unmodeled),
            Err(_) => Err(PredicateIssue::UnreadableComponent),
        }
    }

    pub(crate) fn matches(&self, holder: ComponentHolder<'_>) -> bool {
        match self {
            Self::Equals(expected) => holder.has_equal(expected),
            Self::Unmodeled => false,
        }
    }
}
