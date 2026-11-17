use mcrs_molang::Value;

use crate::entity_query::EntityQuery;

pub trait QuerySource {
    fn query(&self, query: EntityQuery) -> Option<Value>;
}
