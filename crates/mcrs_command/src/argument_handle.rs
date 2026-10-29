use crate::node_id::NodeId;
use core::fmt;
use core::marker::PhantomData;

/// A typed handle to an argument node; the handler reads the value through it.
pub struct ArgumentHandle<T> {
    pub(crate) node: NodeId,
    pub(crate) argument: PhantomData<fn() -> T>,
}

impl<T> ArgumentHandle<T> {
    /// The id of the argument node.
    pub const fn node(self) -> NodeId {
        self.node
    }
}

/// Whether the node holds a `T` is checked when the value is read.
impl<T> From<NodeId> for ArgumentHandle<T> {
    fn from(node: NodeId) -> Self {
        Self {
            node,
            argument: PhantomData,
        }
    }
}

impl<T> From<ArgumentHandle<T>> for NodeId {
    fn from(arg: ArgumentHandle<T>) -> Self {
        arg.node
    }
}

impl<T> Clone for ArgumentHandle<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for ArgumentHandle<T> {}

impl<T> fmt::Debug for ArgumentHandle<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("ArgumentHandle").field(&self.node).finish()
    }
}
