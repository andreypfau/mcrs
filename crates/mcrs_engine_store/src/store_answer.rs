use crate::store_output::StoreOutput;

/// An answer of a [`SharedStore`](crate::SharedStore), for the part `to` that asked.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoreAnswer<R, K, C> {
    pub to: R,
    pub output: StoreOutput<K, C>,
}
