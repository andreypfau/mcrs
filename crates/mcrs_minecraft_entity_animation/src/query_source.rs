use mcrs_molang::Value;

use crate::entity_query::EntityQuery;

/// The values of the entity queries of one entity in one frame. Two calls with the same query in
/// one frame give the same value.
pub trait QuerySource {
    /// The value of `query`, or `None` when it is missing. The host never receives a query that
    /// [`EntityQuery::is_animation_supplied`] marks.
    fn query(&self, query: EntityQuery) -> Option<Value>;
}
