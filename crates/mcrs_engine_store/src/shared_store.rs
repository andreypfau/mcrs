use std::collections::VecDeque;

use mcrs_engine_link::{Link, Placement};

use crate::disk_store::DiskStore;
use crate::store_answer::StoreAnswer;
use crate::store_input::StoreInput;
use crate::store_stage::StoreStage;

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
}

impl<D: DiskStore, R: Copy> SharedStore<D, R> {
    pub fn new(disk: D, placement: Placement) -> Self {
        Self {
            link: Link::new(StoreStage::new(disk), placement),
            askers: VecDeque::new(),
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
        let askers = &mut self.askers;
        self.link
            .outputs()
            .filter_map(move |output| askers.pop_front().map(|to| StoreAnswer { to, output }))
    }

    /// Waits until the data layer has answered every request sent so far, and hands out none of
    /// the answers.
    pub async fn answered(&mut self) {
        self.link.answered().await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chunk_version::ChunkVersion;
    use crate::no_disk::NoDisk;
    use crate::store_output::StoreOutput;
    use crate::versioned_chunk::VersionedChunk;

    #[test]
    fn each_answer_goes_to_the_part_that_asked() {
        for placement in [Placement::Inline, Placement::Thread] {
            let mut store: SharedStore<NoDisk<u32, &'static str>, char> =
                SharedStore::new(NoDisk::new(), placement);
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
}
