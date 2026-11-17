use crate::SharedNetworkState;
use crate::intent::handle_intent;
use crate::packet_io::PacketIo;
use crate::proxy_header::read_proxy_header;
use socket2::SockRef;
use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};
use tokio::io::AsyncRead;
use tokio::net::TcpListener;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use tokio::time::timeout;
use tracing::{error, info, warn};

pub(crate) const HANDLE_CONNECTION_TIMEOUT: Duration = Duration::from_secs(5);

pub const ACCEPT_BUCKET_CAP: u32 = 5;
pub const ACCEPT_REFILL_PER_SEC: f32 = 0.5;
pub const GLOBAL_HANDSHAKE_CAP: usize = 64;
const BUCKET_REFILL_PERIOD: Duration =
    Duration::from_secs((ACCEPT_BUCKET_CAP as f32 / ACCEPT_REFILL_PER_SEC) as u64);

pub struct TokenBucket {
    pub tokens: f32,
    last_refill: Instant,
}

impl TokenBucket {
    pub fn new(initial_tokens: u32) -> Self {
        Self {
            tokens: initial_tokens as f32,
            last_refill: Instant::now(),
        }
    }

    pub fn consume(&mut self, cap: u32, refill_per_sec: f32) -> bool {
        let elapsed = self.last_refill.elapsed().as_secs_f32();
        self.tokens = (self.tokens + elapsed * refill_per_sec).min(cap as f32);
        self.last_refill = Instant::now();
        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false
        }
    }

    pub fn is_full(&self, cap: u32, refill_per_sec: f32) -> bool {
        self.tokens + self.last_refill.elapsed().as_secs_f32() * refill_per_sec >= cap as f32
    }
}

pub(crate) struct AcceptGate {
    buckets: Mutex<Buckets>,
    handshakes: Arc<Semaphore>,
}

struct Buckets {
    per_ip: HashMap<IpAddr, TokenBucket>,
    last_sweep: Instant,
}

impl AcceptGate {
    pub(crate) fn new() -> Self {
        Self {
            buckets: Mutex::new(Buckets {
                per_ip: HashMap::new(),
                last_sweep: Instant::now(),
            }),
            handshakes: Arc::new(Semaphore::new(GLOBAL_HANDSHAKE_CAP)),
        }
    }

    pub(crate) async fn handshake_slot(&self) -> Option<OwnedSemaphorePermit> {
        self.handshakes.clone().acquire_owned().await.ok()
    }

    pub(crate) fn admit(&self, ip: IpAddr) -> bool {
        let mut buckets = self.buckets.lock().unwrap_or_else(PoisonError::into_inner);
        if buckets.last_sweep.elapsed() >= BUCKET_REFILL_PERIOD {
            buckets
                .per_ip
                .retain(|_, bucket| !bucket.is_full(ACCEPT_BUCKET_CAP, ACCEPT_REFILL_PER_SEC));
            if buckets.per_ip.capacity() > 4 * buckets.per_ip.len() {
                buckets.per_ip.shrink_to_fit();
            }
            buckets.last_sweep = Instant::now();
        }
        if ip.is_loopback() {
            return true;
        }
        let bucket = buckets
            .per_ip
            .entry(ip)
            .or_insert_with(|| TokenBucket::new(ACCEPT_BUCKET_CAP));
        let admitted = bucket.consume(ACCEPT_BUCKET_CAP, ACCEPT_REFILL_PER_SEC);
        if !admitted {
            warn!("accept-rate limit exceeded for {ip}");
        }
        admitted
    }
}

pub(crate) async fn start_accept_loop(shared: SharedNetworkState, listener: TcpListener) {
    match listener.local_addr() {
        Ok(address) => info!("Listening on {}", address),
        Err(e) => error!("Failed to read the listener address: {}", e),
    }

    let gate = Arc::new(AcceptGate::new());
    let per_ip_limit = shared.0.per_ip_accept_limit;

    loop {
        let Some(slot) = gate.handshake_slot().await else {
            return;
        };
        match listener.accept().await {
            Ok((mut socket, socket_addr)) => {
                let behind_proxy = shared
                    .0
                    .proxy_upstreams
                    .contains(&socket_addr.ip().to_canonical());
                if !behind_proxy && per_ip_limit && !gate.admit(socket_addr.ip()) {
                    continue;
                }
                let shared = shared.clone();
                let gate = gate.clone();
                tokio::spawn(async move {
                    let _slot = slot;
                    let connection = async {
                        let remote_addr = if behind_proxy {
                            client_behind_proxy(&mut socket, socket_addr, &gate, per_ip_limit).await
                        } else {
                            Some(socket_addr)
                        };
                        if let Some(remote_addr) = remote_addr {
                            handle_connection(shared, socket, remote_addr).await;
                        }
                    };
                    if let Err(e) = timeout(HANDLE_CONNECTION_TIMEOUT, connection).await {
                        warn!("{} Failed to handle connection: {}", socket_addr, e);
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

async fn client_behind_proxy<R: AsyncRead + Unpin>(
    socket: &mut R,
    socket_addr: std::net::SocketAddr,
    gate: &AcceptGate,
    per_ip_limit: bool,
) -> Option<std::net::SocketAddr> {
    match read_proxy_header(socket).await {
        Ok(None) => Some(socket_addr),
        Ok(Some(client)) => (!per_ip_limit || gate.admit(client.ip())).then_some(client),
        Err(e) => {
            warn!("{socket_addr} sent no valid PROXY header: {e}");
            None
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
    if let Err(e) = SockRef::from(&stream).set_send_buffer_size(shared.0.send_buffer_bytes) {
        warn!("Failed to set the send buffer on {}: {}", remote_addr, e);
    }
    let io = PacketIo::new(stream);
    if let Err(e) = handle_intent(shared, io, remote_addr).await {
        warn!("Error during handshake with {}: {}", remote_addr, e);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    #[test]
    fn buckets_full_again_are_dropped_after_a_refill_period() {
        let gate = AcceptGate::new();
        for i in 0..100 {
            gate.admit(IpAddr::V4(Ipv4Addr::new(10, 0, 0, i)));
        }
        assert_eq!(gate.buckets.lock().unwrap().per_ip.len(), 100);

        let earlier = Instant::now() - BUCKET_REFILL_PERIOD;
        let drained = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 0));
        {
            let mut buckets = gate.buckets.lock().unwrap();
            buckets.last_sweep = earlier;
            for bucket in buckets.per_ip.values_mut() {
                bucket.last_refill = earlier;
            }
            let bucket = buckets.per_ip.get_mut(&drained).unwrap();
            bucket.tokens = 0.0;
            bucket.last_refill = Instant::now();
        }

        gate.admit(IpAddr::V4(Ipv4Addr::new(10, 0, 1, 0)));
        let mut kept: Vec<_> = gate
            .buckets
            .lock()
            .unwrap()
            .per_ip
            .keys()
            .copied()
            .collect();
        kept.sort();
        assert_eq!(kept, [drained, IpAddr::V4(Ipv4Addr::new(10, 0, 1, 0))]);
    }

    #[test]
    fn pxp_use_002_the_header_address_is_charged_and_the_upstream_is_not() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        let client = |header: &[u8], gate: &AcceptGate, per_ip_limit: bool| {
            let upstream = "198.51.100.1:40000".parse().unwrap();
            runtime.block_on(client_behind_proxy(
                &mut &header[..],
                upstream,
                gate,
                per_ip_limit,
            ))
        };
        let header =
            |source: &str| format!("PROXY TCP4 {source} 192.0.2.10 51234 25565\r\n").into_bytes();
        let gate = AcceptGate::new();
        let drained: IpAddr = "203.0.113.7".parse().unwrap();
        gate.buckets.lock().unwrap().per_ip.insert(
            drained,
            TokenBucket {
                tokens: 0.0,
                last_refill: Instant::now() + Duration::from_secs(3600),
            },
        );

        assert_eq!(client(&header("203.0.113.7"), &gate, true), None);
        assert_eq!(
            client(&header("203.0.113.7"), &gate, false),
            Some("203.0.113.7:51234".parse().unwrap())
        );
        assert_eq!(
            client(&header("203.0.113.8"), &gate, true),
            Some("203.0.113.8:51234".parse().unwrap())
        );
        assert_eq!(
            client(b"\r\n\r\n\0\r\nQUIT\n\x20\x00\x00\x00", &gate, true),
            Some("198.51.100.1:40000".parse().unwrap())
        );
        let mut charged: Vec<_> = gate
            .buckets
            .lock()
            .unwrap()
            .per_ip
            .keys()
            .copied()
            .collect();
        charged.sort();
        assert_eq!(charged, [drained, "203.0.113.8".parse::<IpAddr>().unwrap()]);
    }

    #[test]
    fn retries_faster_than_one_token_still_refill_the_bucket() {
        let mut bucket = TokenBucket::new(0);
        bucket.last_refill = Instant::now() - Duration::from_secs(1);
        assert!(!bucket.consume(ACCEPT_BUCKET_CAP, ACCEPT_REFILL_PER_SEC));
        bucket.last_refill = Instant::now() - Duration::from_secs(1);
        assert!(bucket.consume(ACCEPT_BUCKET_CAP, ACCEPT_REFILL_PER_SEC));
    }
}
