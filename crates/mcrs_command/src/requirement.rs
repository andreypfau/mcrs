pub trait Requirement<S> {
    fn test(&self, source: &S) -> bool;
}

impl<S> Requirement<S> for () {
    fn test(&self, _: &S) -> bool {
        true
    }
}
