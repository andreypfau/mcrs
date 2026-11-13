use tokio::sync::mpsc;
use tokio::sync::mpsc::error::SendError;
use tokio::sync::mpsc::error::TryRecvError;

use crate::ReceivedPacket;

pub(crate) struct InboundSender {
    pub(crate) packets: mpsc::Sender<ReceivedPacket>,
    credits: mpsc::Receiver<usize>,
    queued: usize,
    limit: usize,
}

pub(crate) struct InboundReceiver {
    packets: mpsc::Receiver<ReceivedPacket>,
    credits: Option<mpsc::Sender<usize>>,
}

pub(crate) fn channel(capacity: usize, limit: usize) -> (InboundSender, InboundReceiver) {
    let (packet_sender, packet_receiver) = mpsc::channel(capacity);
    // Credits cover the queued packets and one send waiting for a packet slot.
    let (credit_sender, credit_receiver) = mpsc::channel(capacity.saturating_add(1));
    (
        InboundSender {
            packets: packet_sender,
            credits: credit_receiver,
            queued: 0,
            limit,
        },
        InboundReceiver {
            packets: packet_receiver,
            credits: Some(credit_sender),
        },
    )
}

impl InboundSender {
    pub(crate) async fn send(
        &mut self,
        packet: ReceivedPacket,
    ) -> Result<(), SendError<ReceivedPacket>> {
        while let Ok(credit) = self.credits.try_recv() {
            self.queued = self.queued.saturating_sub(credit);
        }
        let bytes = packet.payload.len();
        // A legal frame larger than the budget must still fit, on its own.
        while self.queued > 0 && self.queued.saturating_add(bytes) > self.limit {
            let Some(credit) = self.credits.recv().await else {
                return Err(SendError(packet));
            };
            self.queued = self.queued.saturating_sub(credit);
        }
        self.packets.send(packet).await?;
        self.queued = self.queued.saturating_add(bytes);
        Ok(())
    }
}

impl InboundReceiver {
    #[cfg(not(target_family = "wasm"))]
    pub(crate) fn from_packets(packets: mpsc::Receiver<ReceivedPacket>) -> Self {
        Self {
            packets,
            credits: None,
        }
    }

    pub(crate) fn is_closed(&self) -> bool {
        self.packets.is_closed()
    }

    pub(crate) fn try_recv(&mut self) -> Result<ReceivedPacket, TryRecvError> {
        let packet = self.packets.try_recv()?;
        if let Some(credits) = &self.credits
            && credits.try_send(packet.payload.len()).is_err()
        {
            self.credits = None;
        }
        Ok(packet)
    }
}

#[cfg(test)]
#[cfg(not(target_family = "wasm"))]
mod tests {
    use std::future::Future;
    use std::pin::Pin;
    use std::sync::Arc;
    use std::sync::atomic::AtomicBool;
    use std::sync::atomic::Ordering;
    use std::task::Context;
    use std::task::Poll;
    use std::task::Wake;
    use std::task::Waker;

    use bytes::Bytes;

    use super::*;
    use crate::Instant;

    fn packet(id: i32, size: usize) -> ReceivedPacket {
        ReceivedPacket {
            timestamp: Instant::now(),
            id,
            payload: Bytes::from(vec![0; size]),
            verdict: None,
        }
    }

    fn poll<F: Future>(future: Pin<&mut F>) -> Poll<F::Output> {
        future.poll(&mut Context::from_waker(Waker::noop()))
    }

    /// Panics if the packet does not fit both queue bounds.
    fn send_now(sender: &mut InboundSender, packet: ReceivedPacket) {
        assert!(
            matches!(
                poll(Box::pin(sender.send(packet)).as_mut()),
                Poll::Ready(Ok(()))
            ),
            "the packet fits both queue bounds"
        );
    }

    #[derive(Default)]
    struct Woken(AtomicBool);

    impl Wake for Woken {
        fn wake(self: Arc<Self>) {
            self.0.store(true, Ordering::Relaxed);
        }
    }

    #[test]
    fn byte_budget_allows_the_exact_boundary_and_reclaims_dequeued_payloads() {
        let (mut sender, mut receiver) = channel(4, 8);
        send_now(&mut sender, packet(1, 4));
        send_now(&mut sender, packet(2, 4));
        let mut pending = Box::pin(sender.send(packet(3, 1)));
        assert!(
            poll(pending.as_mut()).is_pending(),
            "the payload budget is full"
        );
        assert_eq!(receiver.try_recv().unwrap().id, 1);
        assert!(
            matches!(poll(pending.as_mut()), Poll::Ready(Ok(()))),
            "dequeue returns byte credit"
        );
        assert_eq!(receiver.try_recv().unwrap().id, 2);
        assert_eq!(receiver.try_recv().unwrap().id, 3);
    }

    #[test]
    fn oversized_packets_wait_for_an_empty_queue_and_admit_no_following_payload() {
        let (mut sender, mut receiver) = channel(4, 8);
        send_now(&mut sender, packet(1, 4));
        {
            let mut oversized = Box::pin(sender.send(packet(2, 12)));
            assert!(
                poll(oversized.as_mut()).is_pending(),
                "an oversized frame must be alone"
            );
            assert_eq!(receiver.try_recv().unwrap().id, 1);
            assert!(
                matches!(poll(oversized.as_mut()), Poll::Ready(Ok(()))),
                "one legal large frame fits"
            );
        }
        let mut following = Box::pin(sender.send(packet(3, 1)));
        assert!(
            poll(following.as_mut()).is_pending(),
            "the large frame exhausts the budget"
        );
        assert_eq!(receiver.try_recv().unwrap().id, 2);
        assert!(
            matches!(poll(following.as_mut()), Poll::Ready(Ok(()))),
            "the large frame returned its credit"
        );
        assert_eq!(receiver.try_recv().unwrap().id, 3);
    }

    #[test]
    fn packet_capacity_still_bounds_empty_payloads() {
        let (mut sender, mut receiver) = channel(2, 8);
        send_now(&mut sender, packet(1, 0));
        send_now(&mut sender, packet(2, 0));
        let mut pending = Box::pin(sender.send(packet(3, 0)));
        assert!(
            poll(pending.as_mut()).is_pending(),
            "empty packets still occupy queue slots"
        );
        assert_eq!(receiver.try_recv().unwrap().id, 1);
        assert!(
            matches!(poll(pending.as_mut()), Poll::Ready(Ok(()))),
            "dequeue frees a slot"
        );
        drop(pending);
        assert_eq!(receiver.try_recv().unwrap().id, 2);
        assert_eq!(receiver.try_recv().unwrap().id, 3);
        for id in 4..=5 {
            assert!(
                matches!(
                    poll(Box::pin(sender.send(packet(id, 8))).as_mut()),
                    Poll::Ready(Ok(()))
                ),
                "a drained burst preserves future byte credits"
            );
            assert_eq!(receiver.try_recv().unwrap().id, id);
        }
    }

    #[test]
    fn dropping_the_receiver_ends_sends_waiting_for_bytes_or_slots() {
        for size in [0, 8] {
            let (mut sender, receiver) = channel(1, 8);
            send_now(&mut sender, packet(1, size));
            let mut pending = Box::pin(sender.send(packet(2, 1)));
            let woken = Arc::new(Woken::default());
            let waker = Waker::from(Arc::clone(&woken));
            assert!(
                pending
                    .as_mut()
                    .poll(&mut Context::from_waker(&waker))
                    .is_pending(),
                "the queue is full"
            );
            drop(receiver);
            assert!(
                woken.0.load(Ordering::Relaxed),
                "dropping the receiver wakes the blocked producer"
            );
            let Poll::Ready(Err(error)) = poll(pending.as_mut()) else {
                panic!("a closed receiver must release a blocked producer");
            };
            assert_eq!(error.0.id, 2);
        }
    }

    #[test]
    fn dequeue_wakes_a_send_waiting_for_bytes_or_slots() {
        for size in [0, 8] {
            let (mut sender, mut receiver) = channel(1, 8);
            send_now(&mut sender, packet(1, size));
            let mut pending = Box::pin(sender.send(packet(2, 1)));
            let woken = Arc::new(Woken::default());
            let waker = Waker::from(Arc::clone(&woken));
            assert!(
                pending
                    .as_mut()
                    .poll(&mut Context::from_waker(&waker))
                    .is_pending(),
                "the queue is full"
            );
            assert_eq!(receiver.try_recv().unwrap().id, 1);
            assert!(
                woken.0.load(Ordering::Relaxed),
                "dequeue wakes the blocked producer"
            );
            assert!(
                matches!(poll(pending.as_mut()), Poll::Ready(Ok(()))),
                "the freed capacity admits the packet"
            );
        }
    }

    #[test]
    fn cancelling_a_blocked_send_does_not_consume_the_remaining_budget() {
        let (mut sender, mut receiver) = channel(4, 8);
        send_now(&mut sender, packet(1, 4));
        {
            let mut cancelled = Box::pin(sender.send(packet(2, 8)));
            assert!(
                poll(cancelled.as_mut()).is_pending(),
                "the larger packet has to wait"
            );
        }
        assert!(
            matches!(
                poll(Box::pin(sender.send(packet(3, 4))).as_mut()),
                Poll::Ready(Ok(()))
            ),
            "the cancelled send holds no credit"
        );
        assert_eq!(receiver.try_recv().unwrap().id, 1);
        assert_eq!(receiver.try_recv().unwrap().id, 3);
    }
}
