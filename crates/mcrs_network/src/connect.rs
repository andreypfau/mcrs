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

const HANDLE_CONNECTION_TIMEOUT: Duration = Duration::from_secs(5);

const ACCEPT_BUCKET_CAP: u32 = 5;
// 5 tokens over a 10 s window → 0.5 tokens/s
const ACCEPT_REFILL_PER_SEC: f32 = 0.5;
const GLOBAL_HANDSHAKE_CAP: usize = 64;

pub(crate) struct TokenBucket {
    pub(crate) tokens: u32,
    last_refill: Instant,
}

impl TokenBucket {
    pub(crate) fn new(initial_tokens: u32) -> Self {
        Self {
            tokens: initial_tokens,
            last_refill: Instant::now(),
        }
    }

    pub(crate) fn consume(&mut self, cap: u32, refill_per_sec: f32) -> bool {
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
pub(crate) fn accept_decision(bucket: &mut TokenBucket, inflight: usize) -> AcceptOutcome {
    if !bucket.consume(ACCEPT_BUCKET_CAP, ACCEPT_REFILL_PER_SEC) {
        return AcceptOutcome::RateLimited;
    }
    if inflight >= GLOBAL_HANDSHAKE_CAP {
        return AcceptOutcome::CapExceeded;
    }
    AcceptOutcome::Accept
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum AcceptOutcome {
    Accept,
    RateLimited,
    CapExceeded,
}

/// RAII guard that decrements the in-flight counter on drop and mirrors the
/// updated value to the telemetry global.
struct InflightGuard(Arc<AtomicUsize>);

impl Drop for InflightGuard {
    fn drop(&mut self) {
        let prev = self.0.fetch_sub(1, Ordering::Relaxed);
        BRIDGE_HANDSHAKE_INFLIGHT.store((prev - 1) as u64, Ordering::Relaxed);
    }
}

pub(crate) async fn start_accept_loop(shared: SharedNetworkState) {
    let listener = match TcpListener::bind(shared.0.address).await {
        Ok(listener) => listener,
        Err(e) => {
            error!("Failed to bind to address {} {}", shared.0.address, e);
            return;
        }
    };
    info!("Listening on {}", shared.0.address);

    // HashMap is safe without locks: the accept-loop runs in a single tokio task.
    let mut per_ip_buckets: HashMap<IpAddr, TokenBucket> = HashMap::new();
    let inflight = Arc::new(AtomicUsize::new(0));

    loop {
        match listener.accept().await {
            Ok((socket, remote_addr)) => {
                let ip = remote_addr.ip();
                let bucket = per_ip_buckets
                    .entry(ip)
                    .or_insert_with(|| TokenBucket::new(ACCEPT_BUCKET_CAP));

                let current_inflight = inflight.load(Ordering::Relaxed);
                match accept_decision(bucket, current_inflight) {
                    AcceptOutcome::RateLimited => {
                        warn!("accept-rate limit exceeded for {ip}");
                        // socket dropped here — no tokio task spawned
                        continue;
                    }
                    AcceptOutcome::CapExceeded => {
                        warn!("global handshake cap reached ({current_inflight})");
                        continue;
                    }
                    AcceptOutcome::Accept => {}
                }

                let new_inflight = inflight.fetch_add(1, Ordering::Relaxed) + 1;
                BRIDGE_HANDSHAKE_INFLIGHT.store(new_inflight as u64, Ordering::Relaxed);

                let guard = InflightGuard(inflight.clone());
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
