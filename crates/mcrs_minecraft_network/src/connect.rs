use crate::SharedNetworkState;
use crate::intent::handle_intent;
use crate::metrics::BRIDGE_HANDSHAKE_INFLIGHT;
use crate::packet_io::PacketIo;
use log::{error, info, warn};
use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};
use tokio::net::TcpListener;
use tokio::time::timeout;

pub(crate) const HANDLE_CONNECTION_TIMEOUT: Duration = Duration::from_secs(5);

pub const ACCEPT_BUCKET_CAP: u32 = 5;
// 5 tokens over a 10 s window → 0.5 tokens/s
pub const ACCEPT_REFILL_PER_SEC: f32 = 0.5;
pub const GLOBAL_HANDSHAKE_CAP: usize = 64;

pub struct TokenBucket {
    pub tokens: u32,
    last_refill: Instant,
}

impl TokenBucket {
    pub fn new(initial_tokens: u32) -> Self {
        Self {
            tokens: initial_tokens,
            last_refill: Instant::now(),
        }
    }

    pub fn consume(&mut self, cap: u32, refill_per_sec: f32) -> bool {
        let elapsed = self.last_refill.elapsed().as_secs_f32();
        self.tokens = (self.tokens as f32 + elapsed * refill_per_sec).min(cap as f32) as u32;
        self.last_refill = Instant::now();
        if self.tokens > 0 {
            self.tokens -= 1;
            true
        } else {
            false
        }
    }
}

/// Decision function separated from the async loop so it is testable without a real socket.
pub fn accept_decision(bucket: &mut TokenBucket, inflight: usize) -> AcceptOutcome {
    if !bucket.consume(ACCEPT_BUCKET_CAP, ACCEPT_REFILL_PER_SEC) {
        return AcceptOutcome::RateLimited;
    }
    if inflight >= GLOBAL_HANDSHAKE_CAP {
        return AcceptOutcome::CapExceeded;
    }
    AcceptOutcome::Accept
}

#[derive(Debug, PartialEq, Eq)]
pub enum AcceptOutcome {
    Accept,
    RateLimited,
    CapExceeded,
}

/// RAII guard that decrements the in-flight counter on drop and mirrors the
/// updated value to the telemetry global.
pub(crate) struct InflightGuard(Arc<AtomicUsize>);

impl Drop for InflightGuard {
    fn drop(&mut self) {
        let prev = self.0.fetch_sub(1, Ordering::Relaxed);
        BRIDGE_HANDSHAKE_INFLIGHT.store((prev - 1) as u64, Ordering::Relaxed);
    }
}

/// Shared admission control for every listener: a per-IP token bucket plus one
/// global cap on handshakes still in flight.
pub(crate) struct AcceptGate {
    // HashMap is safe without locks: a gate is owned by a single accept task.
    per_ip_buckets: HashMap<IpAddr, TokenBucket>,
    inflight: Arc<AtomicUsize>,
}

impl AcceptGate {
    pub(crate) fn new() -> Self {
        Self {
            per_ip_buckets: HashMap::new(),
            inflight: Arc::new(AtomicUsize::new(0)),
        }
    }

    pub(crate) fn admit(&mut self, ip: IpAddr) -> Option<InflightGuard> {
        let current_inflight = self.inflight.load(Ordering::Relaxed);

        // The per-IP bucket blunts remote connection floods. An integrated
        // server reconnects over loopback far faster than its 0.5/s refill,
        // where a silent refusal reads as a hang.
        let outcome = if ip.is_loopback() {
            if current_inflight >= GLOBAL_HANDSHAKE_CAP {
                AcceptOutcome::CapExceeded
            } else {
                AcceptOutcome::Accept
            }
        } else {
            let bucket = self
                .per_ip_buckets
                .entry(ip)
                .or_insert_with(|| TokenBucket::new(ACCEPT_BUCKET_CAP));
            accept_decision(bucket, current_inflight)
        };

        match outcome {
            AcceptOutcome::RateLimited => {
                warn!("accept-rate limit exceeded for {ip}");
                return None;
            }
            AcceptOutcome::CapExceeded => {
                warn!("global handshake cap reached ({current_inflight})");
                return None;
            }
            AcceptOutcome::Accept => {}
        }

        let new_inflight = self.inflight.fetch_add(1, Ordering::Relaxed) + 1;
        BRIDGE_HANDSHAKE_INFLIGHT.store(new_inflight as u64, Ordering::Relaxed);
        Some(InflightGuard(self.inflight.clone()))
    }
}

pub(crate) async fn start_accept_loop(shared: SharedNetworkState, listener: TcpListener) {
    match listener.local_addr() {
        Ok(address) => info!("Listening on {}", address),
        Err(e) => error!("Failed to read the listener address: {}", e),
    }

    let mut gate = AcceptGate::new();

    loop {
        match listener.accept().await {
            Ok((socket, remote_addr)) => {
                let Some(guard) = gate.admit(remote_addr.ip()) else {
                    continue;
                };
                let shared = shared.clone();
                tokio::spawn(async move {
                    let _guard = guard;
                    if let Err(e) = timeout(
                        HANDLE_CONNECTION_TIMEOUT,
                        handle_connection(shared, socket, remote_addr),
                    )
                    .await
                    {
                        warn!("{} Failed to handle connection: {}", remote_addr, e);
                    }
                });
            }
            Err(e) => {
                error!("Failed to accept connection: {}", e);
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        }
    }
}

async fn handle_connection(
    shared: SharedNetworkState,
    stream: tokio::net::TcpStream,
    remote_addr: std::net::SocketAddr,
) {
    if let Err(e) = stream.set_nodelay(true) {
        warn!("Failed to set nodelay on {}: {}", remote_addr, e);
    }
    let io = PacketIo::new(stream);
    if let Err(e) = handle_intent(shared, io, remote_addr).await {
        warn!("Error during handshake with {}: {}", remote_addr, e);
    }
}
