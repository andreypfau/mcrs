#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum RegistryValue<I, T> {
    Reference(I),
    Inline(T),
}
