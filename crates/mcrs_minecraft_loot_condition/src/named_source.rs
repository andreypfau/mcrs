use crate::entry_kind::EntryKind;
use crate::named_entries::NamedEntries;

/// What a key of a named entry resolves against in a bake: the named entries of the load, or,
/// while the load bakes them, a list that records each key.
#[derive(Debug)]
pub(crate) enum NamedSource<'a> {
    Baked(&'a NamedEntries),
    Recording(Vec<Dependency>),
}

/// A key in the bake of a named entry: the entry that it names, and the members of the reference
/// chain from the start of that bake up to the key, without the start.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Dependency {
    pub(crate) kind: EntryKind,
    pub(crate) index: usize,
    pub(crate) steps: usize,
}
