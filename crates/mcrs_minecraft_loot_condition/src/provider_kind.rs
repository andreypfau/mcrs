/// The registry of a named provider: `context_int_provider` or `context_float_provider`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProviderKind {
    Int,
    Float,
}
