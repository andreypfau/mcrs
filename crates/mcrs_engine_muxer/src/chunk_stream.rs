use bytes::Bytes;

use crate::entity_id::ViewerId;

const MAX_LOGGED_CHANGES: usize = 256;

#[derive(Clone, Debug)]
pub struct ChunkStream<C> {
    waiting: Vec<ViewerId>,
    holders: Vec<ViewerId>,
    version: Option<StreamVersion<C>>,
    asked: Asked,
}

impl<C> Default for ChunkStream<C> {
    fn default() -> Self {
        Self {
            waiting: Vec::new(),
            holders: Vec::new(),
            version: None,
            asked: Asked::No,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Asked {
    No,
    Yes,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StreamVersion<C> {
    pub bytes: Bytes,
    pub changes: Vec<C>,
}

#[must_use]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StreamNeed {
    Nothing,
    Version,
    Drop,
}

impl<C> ChunkStream<C> {
    #[must_use]
    pub fn holders(&self) -> &[ViewerId] {
        &self.holders
    }

    #[must_use]
    pub fn version(&self) -> Option<&StreamVersion<C>> {
        self.version.as_ref()
    }

    pub fn wait(&mut self, viewer: ViewerId) -> StreamNeed {
        self.waiting.push(viewer);
        self.ask()
    }

    fn ask(&mut self) -> StreamNeed {
        if self.version.is_some() || self.asked == Asked::Yes {
            return StreamNeed::Nothing;
        }
        self.asked = Asked::Yes;
        StreamNeed::Version
    }

    pub fn leave(&mut self, viewer: ViewerId) -> StreamNeed {
        self.waiting.retain(|waiting| *waiting != viewer);
        self.holders.retain(|holder| *holder != viewer);
        self.settle()
    }

    pub fn hold(&mut self, viewer: ViewerId) {
        self.waiting.retain(|waiting| *waiting != viewer);
        self.holders.push(viewer);
        let _ = self.settle();
    }

    fn settle(&mut self) -> StreamNeed {
        if !self.waiting.is_empty() {
            return StreamNeed::Nothing;
        }
        self.version = None;
        if self.holders.is_empty() {
            StreamNeed::Drop
        } else {
            StreamNeed::Nothing
        }
    }

    pub fn publish(&mut self, bytes: Bytes) {
        self.asked = Asked::No;
        if self.waiting.is_empty() {
            return;
        }
        self.version = Some(StreamVersion {
            bytes,
            changes: Vec::new(),
        });
    }
}

impl<C: Clone> ChunkStream<C> {
    pub fn log(&mut self, change: &C) -> StreamNeed {
        let Some(version) = &mut self.version else {
            return StreamNeed::Nothing;
        };
        if version.changes.len() < MAX_LOGGED_CHANGES {
            version.changes.push(change.clone());
            return StreamNeed::Nothing;
        }
        self.version = None;
        self.ask()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_version_with_too_many_changes_is_asked_for_again() {
        let mut stream: ChunkStream<usize> = ChunkStream::default();
        let _ = stream.wait(ViewerId(1));
        stream.publish(Bytes::from_static(b"v"));
        for change in 0..MAX_LOGGED_CHANGES {
            assert_eq!(stream.log(&change), StreamNeed::Nothing);
        }

        assert_eq!(stream.log(&MAX_LOGGED_CHANGES), StreamNeed::Version);
        assert!(stream.version().is_none());
    }
}
