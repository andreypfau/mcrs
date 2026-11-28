use crate::entry_kind::EntryKind;
use crate::named_entries::NamedEntries;

#[derive(Debug)]
pub(crate) enum NamedSource<'a> {
    Baked(&'a NamedEntries),
    Recording(Vec<Dependency>),
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Dependency {
    pub(crate) kind: EntryKind,
    pub(crate) index: usize,
    pub(crate) steps: usize,
}
