use crate::store_output::StoreOutput;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoreAnswer<R, K, C> {
    pub to: R,
    pub output: StoreOutput<K, C>,
}
