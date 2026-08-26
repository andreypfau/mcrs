use mcrs_voxel_math::ColumnPos;
use mcrs_voxel_worldgen::GeneratedChunk;
use std::future::Future;

/// A stored column. The engine never interprets it beyond its sections, so a
/// backend is free to carry whatever else the game asked it to keep.
pub type StoredChunk = GeneratedChunk;

/// Where a column lives between sessions. Anvil is one implementation; empty,
/// backup, a migration wrapper and a key-value store are useful siblings.
pub trait ChunkStore: Send + Sync + 'static {
    type Error: std::error::Error + Send + Sync + 'static;

    fn load(
        &self,
        pos: ColumnPos,
    ) -> impl Future<Output = Result<Option<StoredChunk>, Self::Error>> + Send;

    fn save(
        &self,
        pos: ColumnPos,
        chunk: &StoredChunk,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send;

    fn flush(&self) -> impl Future<Output = Result<(), Self::Error>> + Send {
        async { Ok(()) }
    }
}

/// Reads nothing and keeps nothing. Useful as a default and in tests.
#[derive(Default, Debug, Clone, Copy)]
pub struct EmptyStore;

#[derive(Debug)]
pub enum Never {}

impl std::fmt::Display for Never {
    fn fmt(&self, _: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {}
    }
}

impl std::error::Error for Never {}

impl ChunkStore for EmptyStore {
    type Error = Never;

    async fn load(&self, _pos: ColumnPos) -> Result<Option<StoredChunk>, Never> {
        Ok(None)
    }

    async fn save(&self, _pos: ColumnPos, _chunk: &StoredChunk) -> Result<(), Never> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_empty_store_reads_nothing_back() {
        let store = EmptyStore;
        let loaded = futures_lite_block_on(store.load(ColumnPos::new(0, 0)));
        assert!(loaded.unwrap().is_none(), "an empty store holds no column");
    }

    fn futures_lite_block_on<T>(fut: impl Future<Output = T>) -> T {
        use std::pin::pin;
        use std::task::{Context, Poll, Waker};
        let mut fut = pin!(fut);
        let mut cx = Context::from_waker(Waker::noop());
        loop {
            if let Poll::Ready(v) = fut.as_mut().poll(&mut cx) {
                return v;
            }
        }
    }
}
