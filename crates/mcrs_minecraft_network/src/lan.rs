use std::fmt;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, SocketAddrV4};
use std::time::Duration;
use tokio::net::UdpSocket;
use tokio::sync::mpsc::Sender;
use tokio::task::JoinHandle;
use tokio::time::{MissedTickBehavior, interval};
use tracing::{info, warn};

pub const GROUP: SocketAddrV4 = SocketAddrV4::new(Ipv4Addr::new(224, 0, 2, 60), 4445);
pub const INTERVAL: Duration = Duration::from_millis(1500);
pub const MAX_DATAGRAM: usize = 1024;

const MOTD_CLOSE: &str = "[/MOTD]";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DatagramError {
    ClosesMotd,
    TooLong { len: usize },
}

impl fmt::Display for DatagramError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DatagramError::ClosesMotd => write!(
                f,
                "the MOTD contains {MOTD_CLOSE}, so a vanilla client would drop the announcement"
            ),
            DatagramError::TooLong { len } => write!(
                f,
                "the announcement is {len} bytes, a vanilla client reads {MAX_DATAGRAM}"
            ),
        }
    }
}

impl std::error::Error for DatagramError {}

pub fn announcement(motd: &str, port: u16) -> Result<String, DatagramError> {
    if motd.contains(MOTD_CLOSE) {
        return Err(DatagramError::ClosesMotd);
    }
    let text = format!("[MOTD]{motd}{MOTD_CLOSE}[AD]{port}[/AD]");
    if text.len() > MAX_DATAGRAM {
        return Err(DatagramError::TooLong { len: text.len() });
    }
    Ok(text)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unannounced {
    Off,
    Loopback,
    NoIpv4,
}

/// The game announces over IPv4 only, and its client joins the address the datagram came from,
/// so a listener that accepts no IPv4 would be listed at an address nothing listens on.
pub fn announce_port(bound: SocketAddr, enabled: bool) -> Result<u16, Unannounced> {
    let ip = bound.ip().to_canonical();
    if !enabled {
        Err(Unannounced::Off)
    } else if ip.is_loopback() {
        Err(Unannounced::Loopback)
    } else if !(ip.is_ipv4() || ip.is_unspecified()) {
        Err(Unannounced::NoIpv4)
    } else {
        Ok(bound.port())
    }
}

pub fn source_address(bound: SocketAddr) -> SocketAddr {
    match bound.ip().to_canonical() {
        IpAddr::V4(ip) => SocketAddr::new(ip.into(), 0),
        IpAddr::V6(_) => SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), 0),
    }
}

pub fn enabled(value: Option<&str>) -> bool {
    !matches!(value.map(str::trim), Some("off" | "0" | "false" | "no"))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SendLog {
    Failed,
    Recovered,
}

pub fn after_send(failing: bool, sent: bool) -> (bool, Option<SendLog>) {
    let log = match (failing, sent) {
        (false, false) => Some(SendLog::Failed),
        (true, true) => Some(SendLog::Recovered),
        _ => None,
    };
    (!sent, log)
}

pub async fn announce(
    socket: UdpSocket,
    destination: SocketAddr,
    period: Duration,
    motd: &'static str,
    port: u16,
    stopped: impl Future<Output = ()>,
) {
    let datagram = match announcement(motd, port) {
        Ok(datagram) => datagram,
        Err(reason) => {
            warn!("LAN announcement not sent: {reason}");
            return;
        }
    };
    let mut ticks = interval(period);
    ticks.set_missed_tick_behavior(MissedTickBehavior::Delay);
    let mut stopped = std::pin::pin!(stopped);
    let mut failing = false;
    loop {
        tokio::select! {
            biased;
            () = &mut stopped => return,
            _ = ticks.tick() => {}
        }
        let sent = socket.send_to(datagram.as_bytes(), destination).await;
        let (next, log) = after_send(failing, sent.is_ok());
        failing = next;
        match (log, sent) {
            (Some(SendLog::Failed), Err(error)) => {
                warn!(
                    "LAN announcement to {destination} failed, retrying every {period:?}: {error}"
                )
            }
            (Some(SendLog::Recovered), _) => info!("LAN announcement to {destination} sends again"),
            _ => {}
        }
    }
}

pub(crate) fn start<T: Send + 'static>(
    source: SocketAddr,
    destination: SocketAddr,
    period: Duration,
    motd: &'static str,
    port: u16,
    server: Sender<T>,
) -> std::io::Result<JoinHandle<()>> {
    let socket = std::net::UdpSocket::bind(source)?;
    socket.set_nonblocking(true)?;
    let socket = UdpSocket::from_std(socket)?;
    Ok(tokio::spawn(announce(
        socket,
        destination,
        period,
        motd,
        port,
        async move { server.closed().await },
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn find_from(text: &str, needle: &str, from: usize) -> Option<usize> {
        text[from..].find(needle).map(|at| at + from)
    }

    fn client_text(datagram: &[u8]) -> String {
        let read = &datagram[..datagram.len().min(1024)];
        String::from_utf8_lossy(read).into_owned()
    }

    fn client_motd(text: &str) -> String {
        let Some(start) = text.find("[MOTD]") else {
            return "missing no".to_owned();
        };
        match find_from(text, "[/MOTD]", start + "[MOTD]".len()) {
            Some(end) => text[start + "[MOTD]".len()..end].to_owned(),
            None => "missing no".to_owned(),
        }
    }

    fn client_address(text: &str) -> Option<String> {
        let end_motd = text.find("[/MOTD]")?;
        let after_motd = end_motd + "[/MOTD]".len();
        if find_from(text, "[/MOTD]", after_motd).is_some() {
            return None;
        }
        let start = find_from(text, "[AD]", after_motd)?;
        let end = find_from(text, "[/AD]", start + "[AD]".len())?;
        Some(text[start + "[AD]".len()..end].to_owned())
    }

    fn read_back(motd: &str, port: u16) -> (String, Option<String>) {
        let datagram = announcement(motd, port).unwrap();
        let text = client_text(datagram.as_bytes());
        (client_motd(&text), client_address(&text))
    }

    #[test]
    fn the_datagram_is_the_reference_text_exactly() {
        assert_eq!(
            announcement("A Server", 25565).unwrap(),
            "[MOTD]A Server[/MOTD][AD]25565[/AD]"
        );
    }

    #[test]
    fn a_vanilla_client_reads_the_status_motd_and_the_port_back() {
        let motd = crate::intent::MOTD;
        let datagram = announcement(motd, 25565).unwrap();
        assert!(datagram.len() <= MAX_DATAGRAM);
        assert_eq!(
            read_back(motd, 25565),
            (motd.to_owned(), Some("25565".to_owned()))
        );
    }

    #[test]
    fn two_ports_are_two_addresses() {
        let motd = crate::intent::MOTD;
        let (_, first) = read_back(motd, 25565);
        let (_, second) = read_back(motd, 25566);
        assert!(first.is_some() && second.is_some());
        assert_ne!(first, second);
    }

    #[test]
    fn an_empty_motd_reads_back_empty() {
        assert_eq!(
            read_back("", 25565),
            (String::new(), Some("25565".to_owned()))
        );
    }

    #[test]
    fn a_motd_that_closes_its_own_marker_is_refused() {
        assert_eq!(
            announcement("early[/MOTD]late", 25565),
            Err(DatagramError::ClosesMotd)
        );
        let text = "[MOTD]early[/MOTD]late[/MOTD][AD]25565[/AD]";
        assert_eq!(client_address(text), None);
    }

    #[test]
    fn a_motd_holding_the_other_markers_is_sent_and_reads_back() {
        for motd in ["a[MOTD]b", "a[AD]b", "a[/AD]b"] {
            assert_eq!(
                read_back(motd, 25565),
                (motd.to_owned(), Some("25565".to_owned())),
                "{motd}"
            );
        }
    }

    #[test]
    fn a_datagram_longer_than_the_client_reads_is_refused() {
        let overhead = announcement("", 25565).unwrap().len();
        let room = MAX_DATAGRAM - overhead;

        let exact = "a".repeat(room);
        assert_eq!(announcement(&exact, 25565).unwrap().len(), MAX_DATAGRAM);
        assert_eq!(read_back(&exact, 25565).1, Some("25565".to_owned()));

        let over = "a".repeat(room + 1);
        assert_eq!(
            announcement(&over, 25565),
            Err(DatagramError::TooLong {
                len: MAX_DATAGRAM + 1
            })
        );

        let wide_exact = format!("{}a", "é".repeat((room - 1) / 2));
        assert_eq!(wide_exact.len(), room);
        assert_eq!(
            announcement(&wide_exact, 25565).unwrap().len(),
            MAX_DATAGRAM
        );

        let wide_over = "é".repeat(room / 2 + 1);
        assert!(wide_over.chars().count() < MAX_DATAGRAM);
        assert_eq!(
            announcement(&wide_over, 25565),
            Err(DatagramError::TooLong {
                len: MAX_DATAGRAM + 1
            })
        );
    }

    #[test]
    fn the_destination_and_the_interval_are_the_reference_values() {
        assert_eq!(GROUP.ip(), &Ipv4Addr::new(224, 0, 2, 60));
        assert_eq!(GROUP.port(), 4445);
        assert_eq!(INTERVAL, Duration::from_millis(1500));
        assert_eq!(MAX_DATAGRAM, 1024);
    }

    fn addr(text: &str) -> SocketAddr {
        text.parse().unwrap()
    }

    #[test]
    fn a_loopback_listener_never_announces() {
        for bound in ["127.0.0.1:25565", "[::1]:25565", "[::ffff:127.0.0.1]:25565"] {
            assert_eq!(
                announce_port(addr(bound), true),
                Err(Unannounced::Loopback),
                "{bound}"
            );
            assert_eq!(
                announce_port(addr(bound), false),
                Err(Unannounced::Off),
                "{bound}"
            );
        }
    }

    #[test]
    fn a_network_listener_announces_its_bound_port_unless_the_setting_is_off() {
        for bound in [
            "0.0.0.0:25565",
            "192.168.1.20:25565",
            "[::]:25565",
            "[::ffff:192.168.1.20]:25565",
        ] {
            assert_eq!(announce_port(addr(bound), true), Ok(25565), "{bound}");
            assert_eq!(
                announce_port(addr(bound), false),
                Err(Unannounced::Off),
                "{bound}"
            );
        }
    }

    #[test]
    fn a_listener_on_one_ipv6_address_never_announces() {
        for bound in ["[2001:db8::5]:25565", "[fe80::1]:25565", "[fd00::1]:25565"] {
            assert_eq!(
                announce_port(addr(bound), true),
                Err(Unannounced::NoIpv4),
                "{bound}"
            );
            assert_eq!(
                announce_port(addr(bound), false),
                Err(Unannounced::Off),
                "{bound}"
            );
        }
    }

    #[test]
    fn the_announcement_socket_binds_the_listeners_address() {
        assert_eq!(source_address(addr("0.0.0.0:25565")), addr("0.0.0.0:0"));
        assert_eq!(
            source_address(addr("192.168.1.20:25565")),
            addr("192.168.1.20:0")
        );
        assert_eq!(source_address(addr("[::]:25565")), addr("0.0.0.0:0"));
        assert_eq!(
            source_address(addr("[::ffff:192.168.1.20]:25565")),
            addr("192.168.1.20:0")
        );
    }

    #[test]
    fn the_setting_is_off_only_for_its_off_spellings() {
        for on in [
            None,
            Some(""),
            Some("on"),
            Some("1"),
            Some("yes"),
            Some("anything"),
        ] {
            assert!(enabled(on), "{on:?}");
        }
        for off in ["off", "0", "false", "no", "  off  "] {
            assert!(!enabled(Some(off)), "{off:?}");
        }
    }

    #[test]
    fn a_send_result_is_logged_only_when_the_state_changes() {
        assert_eq!(after_send(false, true), (false, None));
        assert_eq!(after_send(false, false), (true, Some(SendLog::Failed)));
        assert_eq!(after_send(true, false), (true, None));
        assert_eq!(after_send(true, true), (false, Some(SendLog::Recovered)));
    }

    #[test]
    fn a_run_of_failures_logs_once_and_a_recovery_once() {
        let sent = [true, false, false, false, true, true, false];
        let mut failing = false;
        let steps: Vec<Option<SendLog>> = sent
            .into_iter()
            .map(|sent| {
                let (next, log) = after_send(failing, sent);
                failing = next;
                log
            })
            .collect();
        assert_eq!(
            steps,
            [
                None,
                Some(SendLog::Failed),
                None,
                None,
                Some(SendLog::Recovered),
                None,
                Some(SendLog::Failed),
            ]
        );
    }

    use std::time::Instant;
    use tokio::runtime::Runtime;
    use tokio::sync::mpsc;
    use tokio::time::timeout;

    const LIVENESS: Duration = Duration::from_secs(10);
    const SILENCE: Duration = Duration::from_millis(300);
    const FAST: Duration = Duration::from_millis(50);
    const PORT: u16 = 25565;

    async fn loopback() -> UdpSocket {
        UdpSocket::bind("127.0.0.1:0").await.unwrap()
    }

    async fn receive(receiver: &UdpSocket) -> (Vec<u8>, SocketAddr) {
        let mut buffer = [0u8; 2048];
        let (len, from) = timeout(LIVENESS, receiver.recv_from(&mut buffer))
            .await
            .expect("a datagram within the liveness bound")
            .unwrap();
        (buffer[..len].to_vec(), from)
    }

    async fn drain(receiver: &UdpSocket) -> usize {
        let mut buffer = [0u8; 2048];
        let mut drained = 0;
        while timeout(Duration::from_millis(100), receiver.recv_from(&mut buffer))
            .await
            .is_ok()
        {
            drained += 1;
        }
        drained
    }

    async fn assert_silent(receiver: &UdpSocket) {
        let mut buffer = [0u8; 2048];
        assert!(
            timeout(SILENCE, receiver.recv_from(&mut buffer))
                .await
                .is_err(),
            "a datagram arrived"
        );
    }

    fn expected() -> Vec<u8> {
        announcement(crate::intent::MOTD, PORT)
            .unwrap()
            .into_bytes()
    }

    #[test]
    fn the_first_datagram_leaves_at_once_and_the_next_after_the_interval() {
        Runtime::new().unwrap().block_on(async {
            let receiver = loopback().await;
            let task = tokio::spawn(announce(
                loopback().await,
                receiver.local_addr().unwrap(),
                INTERVAL,
                crate::intent::MOTD,
                PORT,
                std::future::pending(),
            ));
            let (first, _) = receive(&receiver).await;
            let first_at = Instant::now();
            let (second, _) = receive(&receiver).await;
            let gap = first_at.elapsed();
            task.abort();
            assert_eq!(first, expected());
            assert_eq!(second, expected());
            assert!(gap >= Duration::from_millis(1400), "{gap:?}");
            assert!(gap < Duration::from_millis(2500), "{gap:?}");
        });
    }

    #[test]
    fn the_announcement_stops_when_the_server_stops() {
        Runtime::new().unwrap().block_on(async {
            let receiver = loopback().await;
            let (server, server_end) = mpsc::channel::<()>(1);
            let task = tokio::spawn(announce(
                loopback().await,
                receiver.local_addr().unwrap(),
                FAST,
                crate::intent::MOTD,
                PORT,
                async move { server.closed().await },
            ));
            receive(&receiver).await;
            receive(&receiver).await;
            drop(server_end);
            timeout(LIVENESS, task)
                .await
                .expect("the task ends once the server is gone")
                .expect("the task does not panic");
            drain(&receiver).await;
            assert_silent(&receiver).await;
        });
    }

    #[test]
    fn a_stop_that_is_already_due_ends_the_task() {
        Runtime::new().unwrap().block_on(async {
            let receiver = loopback().await;
            let task = tokio::spawn(announce(
                loopback().await,
                receiver.local_addr().unwrap(),
                FAST,
                crate::intent::MOTD,
                PORT,
                std::future::ready(()),
            ));
            timeout(LIVENESS, task)
                .await
                .expect("the task ends at once")
                .expect("the task does not panic");
            assert!(drain(&receiver).await <= 1);
        });
    }

    #[test]
    fn a_motd_the_client_cannot_read_sends_nothing() {
        Runtime::new().unwrap().block_on(async {
            let receiver = loopback().await;
            timeout(
                LIVENESS,
                announce(
                    loopback().await,
                    receiver.local_addr().unwrap(),
                    FAST,
                    "early[/MOTD]late",
                    PORT,
                    std::future::pending(),
                ),
            )
            .await
            .expect("the task returns without a stop");
            assert_silent(&receiver).await;
        });
    }

    #[test]
    fn the_started_announcement_sends_from_a_bound_socket_and_stops_with_its_channel() {
        Runtime::new().unwrap().block_on(async {
            let receiver = loopback().await;
            let (server, server_end) = mpsc::channel::<()>(1);
            let task = start(
                "127.0.0.1:0".parse().unwrap(),
                receiver.local_addr().unwrap(),
                FAST,
                crate::intent::MOTD,
                PORT,
                server,
            )
            .unwrap();
            for _ in 0..2 {
                let (datagram, from) = receive(&receiver).await;
                assert_eq!(datagram, expected());
                assert!(from.ip().is_loopback(), "{from}");
            }
            drop(server_end);
            timeout(LIVENESS, task)
                .await
                .expect("the task ends once the channel is closed")
                .expect("the task does not panic");
            drain(&receiver).await;
            assert_silent(&receiver).await;
        });
    }

    #[test]
    fn a_source_that_cannot_be_bound_starts_nothing() {
        Runtime::new().unwrap().block_on(async {
            let receiver = loopback().await;
            let held = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
            let (server, _server_end) = mpsc::channel::<()>(1);
            let started = start(
                held.local_addr().unwrap(),
                receiver.local_addr().unwrap(),
                FAST,
                crate::intent::MOTD,
                PORT,
                server,
            );
            assert_eq!(started.unwrap_err().kind(), std::io::ErrorKind::AddrInUse);
            assert_silent(&receiver).await;
        });
    }
}
