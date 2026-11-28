use alloc::boxed::Box;

#[derive(Clone, Debug, PartialEq)]
pub enum NodeKind<A> {
    Root,
    Literal(Box<str>),
    Argument { name: Box<str>, argument: A },
}

impl<A> NodeKind<A> {
    pub fn name(&self) -> &str {
        match self {
            NodeKind::Root => "",
            NodeKind::Literal(name) | NodeKind::Argument { name, .. } => name,
        }
    }
}
