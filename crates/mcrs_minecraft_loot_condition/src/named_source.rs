use crate::named_providers::NamedProviders;
use crate::provider_kind::ProviderKind;

/// What a key of a provider resolves against in a bake: the named providers of the load, or,
/// while the load bakes them, a list that records each key.
#[derive(Debug)]
pub(crate) enum NamedSource<'a> {
    Baked(&'a NamedProviders),
    Recording(Vec<Dependency>),
}

/// A key of a provider in the bake of a named provider: the entry that it names, and the members
/// of the reference chain from the start of that bake up to the key, without the start.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Dependency {
    pub(crate) kind: ProviderKind,
    pub(crate) index: usize,
    pub(crate) steps: usize,
}
