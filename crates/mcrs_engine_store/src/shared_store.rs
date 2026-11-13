use std::collections::VecDeque;

use mcrs_engine_link::{Link, Placement};

use crate::disk_store::DiskStore;
use crate::store_answer::StoreAnswer;
use crate::store_input::StoreInput;
use crate::store_stage::{StageOutput, StoreStage};

/// One data layer that several parts reach, such as the cells of one world, at the far end of
/// one link. Each answer goes back to the part that asked.
///
/// The stage sees the inputs of all parts in the order they are sent, so a `Get` that one part
/// sends after another part sent a `Put` of the same key sees that `Put`.
pub struct SharedStore<D: DiskStore, R> {
    link: Link<StoreStage<D>>,
    /// The part that sent each `Get` that is not answered yet, oldest first. The stage answers
    /// each `Get` once and in order.
    askers: VecDeque<R>,
    /// The answers that arrived while a flush waited, not handed out yet.
    ready: VecDeque<StoreAnswer<R, D::Key, D::Chunk>>,
    flushes_owed: usize,
    last_flush_ok: bool,
}

impl<D: DiskStore, R: Copy> SharedStore<D, R> {
    pub fn new(disk: D, placement: Placement) -> Self {
        Self {
            link: Link::new(StoreStage::new(disk), placement),
            askers: VecDeque::new(),
            ready: VecDeque::new(),
            flushes_owed: 0,
            last_flush_ok: true,
        }
    }

    pub fn send(&mut self, from: R, input: StoreInput<D::Key, D::Chunk>) {
        if matches!(input, StoreInput::Get { .. }) {
            self.askers.push_back(from);
        }
        self.link.send(input);
    }

    /// Hands out every answer that arrived, in the order of the requests.
    pub fn answers(&mut self) -> impl Iterator<Item = StoreAnswer<R, D::Key, D::Chunk>> + '_ {
        self.collect();
        self.ready.drain(..)
    }

    fn collect(&mut self) {
        for output in self.link.outputs() {
            match output {
                StageOutput::Flushed { ok } => {
                    self.flushes_owed = self.flushes_owed.saturating_sub(1);
                    self.last_flush_ok = ok;
                }
                StageOutput::Answer(output) => {
                    if let Some(to) = self.askers.pop_front() {
                        self.ready.push_back(StoreAnswer { to, output });
                    }
                }
            }
        }
    }

    /// Waits until the data layer has answered every request sent so far, and hands out none of
    /// the answers.
    pub async fn answered(&mut self) {
        self.link.answered().await;
    }

    /// Asks the data layer to write every chunk that it holds, and does not wait for it.
    pub fn request_flush(&mut self) {
        self.flushes_owed += 1;
        self.link.send(StoreInput::Flush);
    }

    /// Waits until the data layer has answered every flush requested so far. Returns whether the
    /// last one made every chunk durable. It is false when the data layer stopped before it
    /// answered.
    pub async fn flushed(&mut self) -> bool {
        self.link.answered().await;
        self.collect();
        self.flushes_owed == 0 && self.last_flush_ok
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chunk_version::ChunkVersion;
    use crate::memory_disk::MemoryDisk;
    use crate::store_output::StoreOutput;
    use crate::versioned_chunk::VersionedChunk;

    #[test]
    fn each_answer_goes_to_the_part_that_asked() {
        for placement in [Placement::Inline, Placement::Thread] {
            let mut store: SharedStore<MemoryDisk<u32, &'static str>, char> =
                SharedStore::new(MemoryDisk::new(), placement);
            let written = VersionedChunk {
                version: ChunkVersion(1),
                chunk: "written by b",
            };
            store.send('a', StoreInput::Get { key: 1 });
            store.send(
                'b',
                StoreInput::Put {
                    key: 1,
                    chunk: written.clone(),
                },
            );
            store.send('b', StoreInput::Get { key: 2 });
            store.send('a', StoreInput::Get { key: 1 });
            bevy_tasks::block_on(store.answered());

            let answers: Vec<StoreAnswer<char, u32, &'static str>> = store.answers().collect();
            assert_eq!(
                answers,
                [
                    StoreAnswer {
                        to: 'a',
                        output: StoreOutput::Missing { key: 1 },
                    },
                    StoreAnswer {
                        to: 'b',
                        output: StoreOutput::Missing { key: 2 },
                    },
                    StoreAnswer {
                        to: 'a',
                        output: StoreOutput::Found {
                            key: 1,
                            chunk: written,
                        },
                    },
                ],
                "{placement:?}"
            );
        }
    }

    #[test]
    fn a_flush_reports_failure_and_leaves_the_askers_in_order() {
        struct Broken;
        impl DiskStore for Broken {
            type Key = u32;
            type Chunk = &'static str;
            type Error = std::io::Error;
            fn read(
                &mut self,
                _: u32,
            ) -> Result<Option<VersionedChunk<&'static str>>, Self::Error> {
                Ok(None)
            }
            fn write(&mut self, _: u32, _: VersionedChunk<&'static str>) {}
            fn flush(&mut self) -> Result<(), Self::Error> {
                Err(std::io::Error::other("full"))
            }
        }
        for placement in [Placement::Inline, Placement::Thread] {
            let mut store: SharedStore<Broken, char> = SharedStore::new(Broken, placement);
            store.send(
                'a',
                StoreInput::Put {
                    key: 1,
                    chunk: VersionedChunk {
                        version: ChunkVersion(1),
                        chunk: "x",
                    },
                },
            );
            store.send('a', StoreInput::Get { key: 2 });
            store.request_flush();
            assert!(!bevy_tasks::block_on(store.flushed()), "{placement:?}");
            let answers: Vec<_> = store.answers().collect();
            assert_eq!(
                answers,
                [StoreAnswer {
                    to: 'a',
                    output: StoreOutput::Missing { key: 2 },
                }],
                "{placement:?}"
            );
        }
    }

    #[test]
    fn a_flush_of_a_memory_store_succeeds() {
        let mut store: SharedStore<MemoryDisk<u32, &'static str>, char> =
            SharedStore::new(MemoryDisk::new(), Placement::Inline);
        store.request_flush();
        assert!(bevy_tasks::block_on(store.flushed()));
    }
}
