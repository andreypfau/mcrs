use alloc::boxed::Box;

/// What a node matches on the command line.
#[derive(Clone, Debug, PartialEq)]
pub enum NodeKind<A> {
    Root,
    Literal(Box<str>),
    Argument { name: Box<str>, argument: A },
}

impl<A> NodeKind<A> {
    /// The literal text or the argument name; empty for the root.
    pub fn name(&self) -> &str {
        match self {
            NodeKind::Root => "",
            NodeKind::Literal(name) | NodeKind::Argument { name, .. } => name,
        }
    }
}
