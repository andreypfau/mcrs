use serde::Deserialize;

use crate::entity_view::EntityView;

#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
pub(crate) struct EntityTagsPredicate {
    any_of: Option<Vec<String>>,
    all_of: Option<Vec<String>>,
    none_of: Option<Vec<String>>,
}

impl EntityTagsPredicate {
    pub(crate) fn matches<W: EntityView>(&self, world: &W, entity: W::Entity) -> bool {
        let has = |tag: &str| world.has_scoreboard_tag(entity, tag);
        self.any_of
            .as_deref()
            .is_none_or(|wanted| wanted.iter().any(|tag| has(tag) == Some(true)))
            && self
                .all_of
                .as_deref()
                .is_none_or(|wanted| wanted.iter().all(|tag| has(tag) == Some(true)))
            && self
                .none_of
                .as_deref()
                .is_none_or(|unwanted| unwanted.iter().all(|tag| has(tag) == Some(false)))
    }
}
