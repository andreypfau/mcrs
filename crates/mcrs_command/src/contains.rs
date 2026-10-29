/// An argument set that holds the argument type `T` as one of its variants.
pub trait Contains<T> {
    /// The argument type, when the set holds this variant.
    fn get(&self) -> Option<&T>;
}
