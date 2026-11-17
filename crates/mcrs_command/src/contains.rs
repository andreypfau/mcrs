pub trait Contains<T> {
    fn get(&self) -> Option<&T>;
}
