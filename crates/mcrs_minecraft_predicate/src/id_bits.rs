use fixedbitset::FixedBitSet;
use mcrs_minecraft_registries::{IdSet, Registries};
use mcrs_minecraft_resource::{Id, Registered};

use crate::predicate_issue::PredicateIssue;

pub(crate) fn members<R: Registered>(
    set: &IdSet<R>,
    registries: &Registries,
    unloaded: PredicateIssue,
) -> Result<Vec<Id<R>>, PredicateIssue> {
    match set {
        IdSet::List(ids) => Ok(ids.to_vec()),
        IdSet::Tag(tag) => {
            let tags = registries.tags::<R>().ok_or(unloaded)?;
            Ok(tags.members(*tag).collect())
        }
    }
}

pub(crate) fn id_bits<R: Registered>(
    set: &IdSet<R>,
    registries: &Registries,
    unloaded: PredicateIssue,
) -> Result<FixedBitSet, PredicateIssue> {
    let ids = members(set, registries, unloaded)?;
    Ok(ids.iter().map(|id| id.index()).collect())
}
