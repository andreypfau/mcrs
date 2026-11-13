use std::collections::VecDeque;

use mcrs_engine_link::{Stage, Step};
use rustc_hash::FxHashMap;
use tracing::error;

use crate::disk_store::DiskStore;
use crate::store_input::StoreInput;
use crate::store_output::StoreOutput;
use crate::versioned_chunk::{VersionedChunk, keep_latest};

/// The number of cooled chunks that the cache may hold before the stage flushes by itself.
/// Between timed flushes this bounds the memory of the cache; the bound is a count and not bytes
/// because each chunk of one kind has a similar size.
pub(crate) const MAX_UNFLUSHED_CHUNKS: usize = 256;

/// What the stage emits: the answer to a `Get`, or the outcome of a `Flush`. `ok` is false when
/// the disk did not make every chunk durable. The disk keeps those chunks for the next flush.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum StageOutput<K, C> {
    Answer(StoreOutput<K, C>),
    Flushed { ok: bool },
}

/// The data layer as a stage: the chunks in memory, by key, over the disk store. It answers the
/// requests in the order it receives them, so a request sent after a put sees that put.
pub(crate) struct StoreStage<D: DiskStore> {
    cache: FxHashMap<D::Key, VersionedChunk<D::Chunk>>,
    disk: D,
    inputs: VecDeque<StoreInput<D::Key, D::Chunk>>,
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
        keep_latest(&mut self.cache, key, chunk);
    }

    fn answer(&mut self, key: D::Key) -> StoreOutput<D::Key, D::Chunk> {
        if let Some(chunk) = self.cache.get(&key) {
            return StoreOutput::Found {
                key,
                chunk: chunk.clone(),
            };
        }
        match self.disk.read(key) {
            Ok(Some(chunk)) => StoreOutput::Found { key, chunk },
            Ok(None) => StoreOutput::Missing { key },
            Err(err) => {
                error!(%err, "the disk store did not read a chunk");
                StoreOutput::Unreadable { key }
            }
        }
    }

    /// Hands every cached version to the disk and flushes it. Returns whether every chunk is
    /// durable. The disk keeps a chunk that it did not make durable for the next flush.
    fn flush(&mut self) -> bool {
        for (key, chunk) in self.cache.drain() {
            self.disk.write(key, chunk);
        }
        match self.disk.flush() {
            Ok(()) => true,
            Err(err) => {
                error!(%err, "the disk store did not flush its chunks");
                false
            }
        }
    }
}

impl<D: DiskStore> Stage for StoreStage<D> {
    const NAME: &'static str = "ChunkStore";

    type Input = StoreInput<D::Key, D::Chunk>;
    type Output = StageOutput<D::Key, D::Chunk>;
    type TaskOutput = ();

    fn receive(&mut self, input: Self::Input) {
        self.inputs.push_back(input);
    }

    fn step(&mut self, step: &mut Step<'_, Self>) {
        while let Some(input) = self.inputs.pop_front() {
            match input {
                StoreInput::Put { key, chunk } => {
                    self.put(key, chunk);
                    if self.cache.len() >= MAX_UNFLUSHED_CHUNKS {
                        self.flush();
                    }
                }
                StoreInput::Get { key } => step.emit(StageOutput::Answer(self.answer(key))),
                StoreInput::Flush => {
                    let ok = self.flush();
                    step.emit(StageOutput::Flushed { ok });
                }
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
    use mcrs_engine_link::{Link, Placement};
    use rustc_hash::FxHashMap;

    /// A disk that holds what it is handed until a flush moves it into `chunks`.
    #[derive(Default)]
    struct MapDisk {
        chunks: FxHashMap<u32, VersionedChunk<&'static str>>,
        pending: FxHashMap<u32, VersionedChunk<&'static str>>,
        flush_fails: bool,
        flushes: usize,
    }

    #[derive(Debug)]
    struct Failure;

    impl std::fmt::Display for Failure {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("flush")
        }
    }

    impl DiskStore for MapDisk {
        type Key = u32;
        type Chunk = &'static str;
        type Error = Failure;

        fn read(&mut self, key: u32) -> Result<Option<VersionedChunk<&'static str>>, Failure> {
            Ok(self
                .pending
                .get(&key)
                .or_else(|| self.chunks.get(&key))
                .cloned())
        }

        fn write(&mut self, key: u32, chunk: VersionedChunk<&'static str>) {
            keep_latest(&mut self.pending, key, chunk);
        }

        fn flush(&mut self) -> Result<(), Failure> {
            self.flushes += 1;
            if self.flush_fails {
                return Err(Failure);
            }
            self.chunks.extend(self.pending.drain());
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
    ) -> Vec<StageOutput<u32, &'static str>> {
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
                    StageOutput::Answer(StoreOutput::Missing { key: 1 }),
                    StageOutput::Answer(StoreOutput::Found {
                        key: 1,
                        chunk: version(2, "second"),
                    }),
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
            [StageOutput::Answer(StoreOutput::Found {
                key: 7,
                chunk: version(3, "saved"),
            })]
        );
    }

    #[test]
    fn a_flush_writes_the_unwritten_versions_to_the_disk_and_clears_them() {
        let mut end = mcrs_engine_link::InlineEnd::new(StoreStage::new(MapDisk::default()));
        end.receive(put(4, 1, "cold"));
        end.receive(StoreInput::Flush);
        let _ = end.step();
        assert_eq!(end.stage().disk.chunks.get(&4), Some(&version(1, "cold")));
        assert!(end.stage().cache.is_empty());
        assert_eq!(end.stage().disk.flushes, 1);
    }

    #[test]
    fn a_get_does_not_keep_a_chunk_that_only_the_disk_holds() {
        let mut disk = MapDisk::default();
        disk.chunks.insert(7, version(3, "first"));
        let mut end = mcrs_engine_link::InlineEnd::new(StoreStage::new(disk));
        end.receive(StoreInput::Get { key: 7 });
        let first: Vec<_> = end.step().collect();
        end.stage_mut().disk.chunks.insert(7, version(4, "second"));
        end.receive(StoreInput::Get { key: 7 });
        let second: Vec<_> = end.step().collect();
        assert_eq!(
            first,
            [StageOutput::Answer(StoreOutput::Found {
                key: 7,
                chunk: version(3, "first"),
            })]
        );
        assert_eq!(
            second,
            [StageOutput::Answer(StoreOutput::Found {
                key: 7,
                chunk: version(4, "second"),
            })]
        );
    }

    #[test]
    fn a_flush_that_fails_leaves_the_chunks_with_the_disk_for_the_next_try() {
        let disk = MapDisk {
            flush_fails: true,
            ..MapDisk::default()
        };
        let mut end = mcrs_engine_link::InlineEnd::new(StoreStage::new(disk));
        end.receive(put(4, 1, "cold"));
        end.receive(StoreInput::Flush);
        end.receive(StoreInput::Get { key: 4 });
        let failed: Vec<_> = end.step().collect();
        assert_eq!(
            failed,
            [
                StageOutput::Flushed { ok: false },
                StageOutput::Answer(StoreOutput::Found {
                    key: 4,
                    chunk: version(1, "cold"),
                }),
            ]
        );
        assert!(end.stage().cache.is_empty());

        end.stage_mut().disk.flush_fails = false;
        end.receive(StoreInput::Flush);
        let retried: Vec<_> = end.step().collect();
        assert_eq!(retried, [StageOutput::Flushed { ok: true }]);
        assert_eq!(end.stage().disk.chunks.get(&4), Some(&version(1, "cold")));
    }

    #[test]
    fn a_late_older_put_does_not_replace_a_version_that_a_failed_flush_left_with_the_disk() {
        let disk = MapDisk {
            flush_fails: true,
            ..MapDisk::default()
        };
        let mut end = mcrs_engine_link::InlineEnd::new(StoreStage::new(disk));
        end.receive(put(4, 2, "new"));
        end.receive(StoreInput::Flush);
        end.receive(put(4, 1, "old"));
        end.receive(StoreInput::Flush);
        let _ = end.step().collect::<Vec<_>>();

        end.stage_mut().disk.flush_fails = false;
        end.receive(StoreInput::Flush);
        let _ = end.step().collect::<Vec<_>>();
        assert_eq!(end.stage().disk.chunks.get(&4), Some(&version(2, "new")));
    }

    #[test]
    fn a_memory_disk_keeps_its_chunks_through_a_flush() {
        let mut end = mcrs_engine_link::InlineEnd::new(StoreStage::new(crate::MemoryDisk::<
            u32,
            &'static str,
            Failure,
        >::new()));
        end.receive(put(4, 1, "edit"));
        end.receive(StoreInput::Flush);
        end.receive(StoreInput::Get { key: 4 });
        let outputs: Vec<_> = end.step().collect();
        assert_eq!(
            outputs,
            [
                StageOutput::Flushed { ok: true },
                StageOutput::Answer(StoreOutput::Found {
                    key: 4,
                    chunk: version(1, "edit"),
                }),
            ]
        );
    }

    #[test]
    fn the_cache_flushes_by_itself_when_it_passes_the_bound() {
        let mut end = mcrs_engine_link::InlineEnd::new(StoreStage::new(MapDisk::default()));
        for key in 0..MAX_UNFLUSHED_CHUNKS as u32 {
            end.receive(put(key, 1, "cold"));
        }
        let outputs: Vec<_> = end.step().collect();
        assert!(outputs.is_empty());
        assert!(end.stage().cache.is_empty());
        assert_eq!(end.stage().disk.chunks.len(), MAX_UNFLUSHED_CHUNKS);
    }

    #[test]
    fn a_failing_disk_is_not_flushed_again_for_every_put_over_the_bound() {
        let disk = MapDisk {
            flush_fails: true,
            ..MapDisk::default()
        };
        let mut end = mcrs_engine_link::InlineEnd::new(StoreStage::new(disk));
        for key in 0..MAX_UNFLUSHED_CHUNKS as u32 + 10 {
            end.receive(put(key, 1, "cold"));
        }
        let _ = end.step();
        assert_eq!(end.stage().disk.flushes, 1);
    }
}
