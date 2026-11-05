use bytes::Bytes;

use crate::entity_id::ViewerId;

/// A version that waits for more changes than this is dropped, and a later one is asked for:
/// a viewer that takes a chunk much later gets a fresh version rather than a long replay.
const MAX_LOGGED_CHANGES: usize = 256;

/// The version stream of one chunk, as the muxer holds it for the viewers that hold the chunk
/// or wait for it.
///
/// The owner of the chunk encodes a version once, and every viewer that takes it gets the same
/// bytes. A version holds the changes of the ticks before the tick that published it. The
/// stream keeps each change after the version for the viewers that wait, so a viewer gets the
/// version and then those changes. A change that the version holds already does no harm when
/// it comes again.
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

/// Whether a version that the owner did not publish yet was asked for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Asked {
    No,
    Yes,
}

/// A version of a chunk and the changes that came after it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StreamVersion<C> {
    /// The version, encoded once by its owner.
    pub bytes: Bytes,
    pub changes: Vec<C>,
}

/// What a stream asks of the owner of its chunk after a change of its viewers.
#[must_use]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StreamNeed {
    Nothing,
    /// A version, for the viewers that wait.
    Version,
    /// No viewer holds the chunk or waits for it: the stream goes.
    Drop,
}

impl<C> ChunkStream<C> {
    /// The viewers that hold the chunk, which get each change of it.
    #[must_use]
    pub fn holders(&self) -> &[ViewerId] {
        &self.holders
    }

    /// The last version, while viewers wait for it.
    #[must_use]
    pub fn version(&self) -> Option<&StreamVersion<C>> {
        self.version.as_ref()
    }

    /// `viewer` waits for the chunk.
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

    /// `viewer` stops waiting for the chunk or holding it.
    pub fn leave(&mut self, viewer: ViewerId) -> StreamNeed {
        self.waiting.retain(|waiting| *waiting != viewer);
        self.holders.retain(|holder| *holder != viewer);
        self.settle()
    }

    /// `viewer` got the chunk, and holds it from now on.
    pub fn hold(&mut self, viewer: ViewerId) {
        self.waiting.retain(|waiting| *waiting != viewer);
        self.holders.push(viewer);
        // A holder keeps the stream, so it never goes here.
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

    /// Takes in a version that the owner published, for the viewers that wait.
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
    /// Keeps `change` for the viewers that wait for the version. The holders get it apart.
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
