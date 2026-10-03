use std::fmt;
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::time::Duration;

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

pub fn announce_port(_bound: SocketAddr, _enabled: bool) -> Option<u16> {
    None
}

pub fn source_address(bound: SocketAddr) -> SocketAddr {
    bound
}

pub fn enabled(_value: Option<&str>) -> bool {
    true
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SendLog {
    Failed,
    Recovered,
}

pub fn after_send(_failing: bool, _sent: bool) -> (bool, Option<SendLog>) {
    (false, None)
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
        for bound in ["127.0.0.1:25565", "[::1]:25565"] {
            for setting in [true, false] {
                assert_eq!(
                    announce_port(addr(bound), setting),
                    None,
                    "{bound} {setting}"
                );
            }
        }
    }

    #[test]
    fn a_network_listener_announces_its_bound_port_unless_the_setting_is_off() {
        for bound in ["0.0.0.0:25565", "192.168.1.20:25565", "[::]:25565"] {
            assert_eq!(announce_port(addr(bound), true), Some(25565), "{bound}");
            assert_eq!(announce_port(addr(bound), false), None, "{bound}");
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
}
