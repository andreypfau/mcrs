/// A predicate over the source of a command, held as data so the host can also inspect it.
pub trait Requirement<S> {
    /// Whether the source may use the node.
    fn test(&self, source: &S) -> bool;
}

/// No requirement: every source passes.
impl<S> Requirement<S> for () {
    fn test(&self, _: &S) -> bool {
        true
    }
}
