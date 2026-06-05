use bytes::Bytes;
use tokio::sync::mpsc;

/// A no-socket send sink backed by a bounded channel. The sender is stored
/// here; the receiver is returned to the caller so tests can assert exactly
/// which coalesced blobs arrive and in what order.
pub struct MockSink {
    pub tx: mpsc::Sender<Bytes>,
}

impl MockSink {
    pub async fn send(&self, blob: Bytes) -> Result<(), mpsc::error::SendError<Bytes>> {
        self.tx.send(blob).await
    }

    pub fn try_send(&self, blob: Bytes) -> Result<(), mpsc::error::TrySendError<Bytes>> {
        self.tx.try_send(blob)
    }
}

/// Returns a `(MockSink, Receiver)` pair. The `MockSink` stands in for the
/// bounded outgoing channel inside `RawConnection`; the `Receiver` gives the
/// test visibility into every blob that would have been written to TCP.
pub fn make_mock_sink(capacity: usize) -> (MockSink, mpsc::Receiver<Bytes>) {
    let (tx, rx) = mpsc::channel(capacity);
    (MockSink { tx }, rx)
}
