use std::mem;

use bytes::Bytes;
use mcrs_minecraft_engine_world::session::PlayerSession;

use crate::column_change::ColumnChange;
use crate::column_packet::{DirectBits, encode_stored_column};
use crate::column_version::ColumnVersion;

/// A version that waits for more changes than this is dropped, and a later one is asked for:
/// a client that takes a column much later gets a fresh version rather than a long replay.
const MAX_LOGGED_CHANGES: usize = 256;

/// The version stream of one column of a dimension, as the muxer holds it for the clients that
/// hold the column or wait for it.
#[derive(Default)]
pub(crate) struct ColumnFeed {
    /// The clients that wait for the column.
    pub(crate) waiting: Vec<PlayerSession>,
    /// The clients that hold the column, which get each change of it.
    pub(crate) holders: Vec<PlayerSession>,
    /// The last version, while clients wait for it.
    pub(crate) version: Option<FeedVersion>,
    asked: Asked,
}

/// Whether a version that the dimension did not publish yet was asked for.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Asked {
    #[default]
    No,
    Yes,
}

/// A version of a column and the changes that came after it.
pub(crate) struct FeedVersion {
    pub(crate) encoding: Encoding,
    pub(crate) changes: Vec<ColumnChange>,
}

/// The packet of a version, encoded once for every client that takes it.
pub(crate) enum Encoding {
    Unencoded(Box<ColumnVersion>),
    Encoded(Bytes),
    /// The encode logged why; the column stays unsent rather than produced again.
    Failed,
}

impl Encoding {
    /// Encodes the version, if it is not encoded yet.
    pub(crate) fn encode(&mut self, direct_bits: DirectBits) {
        if !matches!(self, Self::Unencoded(_)) {
            return;
        }
        let Self::Unencoded(version) = mem::replace(self, Self::Failed) else {
            return;
        };
        if let Some(packet) =
            encode_stored_column(version.column, &version.stored, direct_bits, version.light)
        {
            *self = Self::Encoded(packet);
        }
    }

    #[must_use]
    pub(crate) fn is_sendable(&self) -> bool {
        !matches!(self, Self::Failed)
    }
}

/// What a feed asks of the dimension after a change of its clients.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FeedNeed {
    Nothing,
    /// A version, for the clients that wait.
    Version,
    /// No client holds the column or waits for it: the feed goes.
    Drop,
}

impl ColumnFeed {
    /// `session` waits for the column.
    pub(crate) fn wait(&mut self, session: PlayerSession) -> FeedNeed {
        self.waiting.push(session);
        self.ask()
    }

    fn ask(&mut self) -> FeedNeed {
        if self.version.is_some() || self.asked == Asked::Yes {
            return FeedNeed::Nothing;
        }
        self.asked = Asked::Yes;
        FeedNeed::Version
    }

    /// `session` stops waiting for the column or holding it.
    pub(crate) fn leave(&mut self, session: PlayerSession) -> FeedNeed {
        self.waiting.retain(|waiting| *waiting != session);
        self.holders.retain(|holder| *holder != session);
        self.settle()
    }

    /// `session` got the column, and holds it from now on.
    pub(crate) fn hold(&mut self, session: PlayerSession) {
        self.waiting.retain(|waiting| *waiting != session);
        self.holders.push(session);
        self.settle();
    }

    fn settle(&mut self) -> FeedNeed {
        if !self.waiting.is_empty() {
            return FeedNeed::Nothing;
        }
        self.version = None;
        if self.holders.is_empty() {
            FeedNeed::Drop
        } else {
            FeedNeed::Nothing
        }
    }

    /// Takes in a version that the dimension published, for the clients that wait.
    pub(crate) fn publish(&mut self, version: ColumnVersion) {
        self.asked = Asked::No;
        if self.waiting.is_empty() {
            return;
        }
        self.version = Some(FeedVersion {
            encoding: Encoding::Unencoded(Box::new(version)),
            changes: Vec::new(),
        });
    }

    /// Keeps `change` for the clients that wait for the version. The holders get it apart.
    pub(crate) fn log(&mut self, change: &ColumnChange) -> FeedNeed {
        let Some(version) = &mut self.version else {
            return FeedNeed::Nothing;
        };
        if version.changes.len() < MAX_LOGGED_CHANGES {
            version.changes.push(change.clone());
            return FeedNeed::Nothing;
        }
        self.version = None;
        self.ask()
    }
}
