use std::collections::VecDeque;

use mcrs_engine_link::{Stage, Step};
use rustc_hash::FxHashMap;
use tracing::error;

use crate::disk_store::DiskStore;
use crate::store_input::StoreInput;
use crate::store_output::StoreOutput;
use crate::versioned_chunk::VersionedChunk;

/// The data layer as a stage: the chunks in memory, by key, over the disk store. It answers the
/// requests in the order it receives them, so a request sent after a put sees that put.
pub(crate) struct StoreStage<D: DiskStore> {
    cache: FxHashMap<D::Key, Cached<D::Chunk>>,
    disk: D,
    inputs: VecDeque<StoreInput<D::Key, D::Chunk>>,
}

struct Cached<C> {
    chunk: VersionedChunk<C>,
    disk: DiskState,
}

/// Whether the disk holds the version that the cache holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DiskState {
    Written,
    Unwritten,
}

impl<D: DiskStore> StoreStage<D> {
    pub(crate) fn new(disk: D) -> Self {
        Self {
            cache: FxHashMap::default(),
            disk,
            inputs: VecDeque::new(),
        }
    }

    /// Keeps `chunk` as the version of `key` that the disk does not hold yet. It keeps a later
    /// version that it already holds instead.
    fn put(&mut self, key: D::Key, chunk: VersionedChunk<D::Chunk>) {
        if self
            .cache
            .get(&key)
            .is_some_and(|held| held.chunk.version > chunk.version)
        {
            return;
        }
        let disk = DiskState::Unwritten;
        self.cache.insert(key, Cached { chunk, disk });
    }

    fn answer(&mut self, key: D::Key) -> StoreOutput<D::Key, D::Chunk> {
        if let Some(cached) = self.cache.get(&key) {
            return StoreOutput::Found {
                key,
                chunk: cached.chunk.clone(),
            };
        }
        match self.disk.read(key) {
            Ok(Some(chunk)) => {
                let cached = Cached {
                    chunk: chunk.clone(),
                    disk: DiskState::Written,
                };
                self.cache.insert(key, cached);
                StoreOutput::Found { key, chunk }
            }
            Ok(None) => StoreOutput::Missing { key },
            Err(err) => {
                error!(%err, "the disk store did not read a chunk");
                StoreOutput::Unreadable { key }
            }
        }
    }

    /// Writes each version that the disk does not hold yet.
    fn flush(&mut self) {
        for (key, cached) in &mut self.cache {
            if cached.disk == DiskState::Written {
                continue;
            }
            match self.disk.write(*key, &cached.chunk) {
                Ok(()) => cached.disk = DiskState::Written,
                Err(err) => error!(%err, "the disk store did not write a chunk"),
            }
        }
    }
}

impl<D: DiskStore> Stage for StoreStage<D> {
    const NAME: &'static str = "ChunkStore";

    type Input = StoreInput<D::Key, D::Chunk>;
    type Output = StoreOutput<D::Key, D::Chunk>;
    type TaskOutput = ();

    fn receive(&mut self, input: Self::Input) {
        self.inputs.push_back(input);
    }

    fn step(&mut self, step: &mut Step<'_, Self>) {
        while let Some(input) = self.inputs.pop_front() {
            match input {
                StoreInput::Put { key, chunk } => self.put(key, chunk),
                StoreInput::Get { key } => step.emit(self.answer(key)),
                StoreInput::Flush => self.flush(),
            }
        }
    }

    fn has_work(&self) -> bool {
        !self.inputs.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chunk_version::ChunkVersion;
    use core::convert::Infallible;
    use mcrs_engine_link::{Link, Placement};
    use rustc_hash::FxHashMap;

    /// A disk that keeps what it is handed in a map.
    #[derive(Default)]
    struct MapDisk {
        chunks: FxHashMap<u32, VersionedChunk<&'static str>>,
    }

    impl DiskStore for MapDisk {
        type Key = u32;
        type Chunk = &'static str;
        type Error = Infallible;

        fn read(&mut self, key: u32) -> Result<Option<VersionedChunk<&'static str>>, Infallible> {
            Ok(self.chunks.get(&key).cloned())
        }

        fn write(
            &mut self,
            key: u32,
            chunk: &VersionedChunk<&'static str>,
        ) -> Result<(), Infallible> {
            self.chunks.insert(key, chunk.clone());
            Ok(())
        }
    }

    fn version(number: u64, chunk: &'static str) -> VersionedChunk<&'static str> {
        VersionedChunk {
            version: ChunkVersion(number),
            chunk,
        }
    }

    fn put(key: u32, number: u64, chunk: &'static str) -> StoreInput<u32, &'static str> {
        StoreInput::Put {
            key,
            chunk: version(number, chunk),
        }
    }

    fn answers(
        placement: Placement,
        disk: MapDisk,
        inputs: Vec<StoreInput<u32, &'static str>>,
    ) -> Vec<StoreOutput<u32, &'static str>> {
        let mut link = Link::new(StoreStage::new(disk), placement);
        for input in inputs {
            link.send(input);
        }
        bevy_tasks::block_on(link.answered());
        link.outputs().collect()
    }

    #[test]
    fn a_get_sees_the_latest_put_before_it_and_never_an_older_one() {
        for placement in [Placement::Inline, Placement::Thread] {
            let outputs = answers(
                placement,
                MapDisk::default(),
                vec![
                    StoreInput::Get { key: 1 },
                    put(1, 2, "second"),
                    put(1, 1, "first, late"),
                    StoreInput::Get { key: 1 },
                ],
            );
            assert_eq!(
                outputs,
                [
                    StoreOutput::Missing { key: 1 },
                    StoreOutput::Found {
                        key: 1,
                        chunk: version(2, "second"),
                    },
                ],
                "{placement:?}"
            );
        }
    }

    #[test]
    fn a_chunk_that_only_the_disk_holds_is_read_from_it() {
        let mut disk = MapDisk::default();
        disk.chunks.insert(7, version(3, "saved"));
        let outputs = answers(Placement::Inline, disk, vec![StoreInput::Get { key: 7 }]);
        assert_eq!(
            outputs,
            [StoreOutput::Found {
                key: 7,
                chunk: version(3, "saved"),
            }]
        );
    }

    #[test]
    fn a_flush_writes_the_unwritten_versions_to_the_disk() {
        let mut end = mcrs_engine_link::InlineEnd::new(StoreStage::new(MapDisk::default()));
        end.receive(put(4, 1, "cold"));
        end.receive(StoreInput::Flush);
        let _ = end.step();
        assert_eq!(end.stage().disk.chunks.get(&4), Some(&version(1, "cold")));
    }
}
