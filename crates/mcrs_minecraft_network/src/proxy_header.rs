use std::io::{Error, ErrorKind, Result};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use tokio::io::{AsyncRead, AsyncReadExt};

const V2_SIGNATURE: [u8; 12] = *b"\r\n\r\n\0\r\nQUIT\n";
const V1_SIGNATURE: &[u8] = b"PROXY ";
const V1_MAX_LEN: usize = 107;

pub(crate) async fn read_proxy_header<R: AsyncRead + Unpin>(
    stream: &mut R,
) -> Result<Option<SocketAddr>> {
    let mut start = [0; 12];
    stream.read_exact(&mut start).await?;
    if start == V2_SIGNATURE {
        let mut fixed = [0; 4];
        stream.read_exact(&mut fixed).await?;
        let mut rest = vec![0; usize::from(u16::from_be_bytes([fixed[2], fixed[3]]))];
        stream.read_exact(&mut rest).await?;
        return parse_v2(fixed[0], fixed[1], &rest).ok_or_else(malformed);
    }
    if !start.starts_with(V1_SIGNATURE) {
        return Err(malformed());
    }
    if start.contains(&b'\n') {
        return Err(malformed());
    }
    let mut line = start.to_vec();
    while line.last() != Some(&b'\n') {
        if line.len() == V1_MAX_LEN {
            return Err(malformed());
        }
        line.push(stream.read_u8().await?);
    }
    parse_v1(&line).ok_or_else(malformed)
}

fn malformed() -> Error {
    Error::new(ErrorKind::InvalidData, "malformed PROXY header")
}

fn parse_v1(line: &[u8]) -> Option<Option<SocketAddr>> {
    let line = line.strip_suffix(b"\r\n")?;
    if line.contains(&b'\r') {
        return None;
    }
    if line == b"PROXY UNKNOWN" || line.starts_with(b"PROXY UNKNOWN ") {
        return Some(None);
    }
    let line = std::str::from_utf8(line).ok()?;
    let fields: Vec<&str> = line.split(' ').collect();
    let [
        "PROXY",
        family,
        source,
        destination,
        source_port,
        destination_port,
    ] = fields[..]
    else {
        return None;
    };
    let address = |text| match family {
        "TCP4" => ipv4(text).map(IpAddr::V4),
        "TCP6" => ipv6(text).map(IpAddr::V6),
        _ => None,
    };
    let ip = address(source)?;
    address(destination)?;
    decimal(destination_port)?;
    Some(Some(SocketAddr::new(
        ip.to_canonical(),
        decimal(source_port)?,
    )))
}

fn decimal(text: &str) -> Option<u16> {
    let digits = text.bytes().all(|byte| byte.is_ascii_digit());
    (digits && (text == "0" || !text.starts_with('0')))
        .then(|| text.parse().ok())
        .flatten()
}

fn ipv4(text: &str) -> Option<Ipv4Addr> {
    text.split('.')
        .try_for_each(|part| decimal(part).map(drop))?;
    text.parse().ok()
}

fn ipv6(text: &str) -> Option<Ipv6Addr> {
    let tail = text.rsplit(':').next()?;
    if tail.contains('.') {
        ipv4(tail)?;
    }
    text.parse().ok()
}

fn parse_v2(version_command: u8, family: u8, rest: &[u8]) -> Option<Option<SocketAddr>> {
    match version_command {
        0x20 => return Some(None),
        0x21 => {}
        _ => return None,
    }
    match family {
        0x11 => {
            let block: &[u8; 12] = rest.get(..12)?.try_into().ok()?;
            let ip = Ipv4Addr::new(block[0], block[1], block[2], block[3]);
            let port = u16::from_be_bytes([block[8], block[9]]);
            Some(Some(SocketAddr::new(IpAddr::V4(ip), port)))
        }
        0x21 => {
            let block: &[u8; 36] = rest.get(..36)?.try_into().ok()?;
            let source: [u8; 16] = block[..16].try_into().ok()?;
            let ip = IpAddr::V6(Ipv6Addr::from(source)).to_canonical();
            Some(Some(SocketAddr::new(
                ip,
                u16::from_be_bytes([block[32], block[33]]),
            )))
        }
        0x00 | 0x12 | 0x22 | 0x31 | 0x32 => Some(None),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(text: &str) -> Vec<u8> {
        text.split_whitespace()
            .map(|pair| u8::from_str_radix(pair, 16).unwrap())
            .collect()
    }

    const BLOCK_A: &str = "0d 0a 0d 0a 00 0d 0a 51 55 49 54 0a 21 11 00 13
        cb 00 71 07 c0 00 02 0a c8 22 63 dd 03 00 04 00 00 00 00";
    const BLOCK_B: &str = "0d 0a 0d 0a 00 0d 0a 51 55 49 54 0a 20 00 00 00";
    const BLOCK_C: &str = "0d 0a 0d 0a 00 0d 0a 51 55 49 54 0a 11 11 00 0c
        cb 00 71 07 c0 00 02 0a c8 22 63 dd";
    const BLOCK_D: &str = "0d 0a 0d 0a 00 0d 0a 51 55 49 54 0a 21 11 00 08
        cb 00 71 07 c0 00 02 0a";

    fn read(input: &[u8]) -> (Result<Option<SocketAddr>>, usize) {
        let mut stream = input;
        let result = block_on(read_proxy_header(&mut stream));
        (result, input.len() - stream.len())
    }

    fn block_on<F: std::future::Future>(future: F) -> F::Output {
        tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap()
            .block_on(future)
    }

    fn client(text: &str) -> Option<SocketAddr> {
        Some(text.parse().unwrap())
    }

    #[test]
    fn valid_headers_give_their_client_address_and_consume_only_the_header() {
        let v1_tcp4 = b"PROXY TCP4 203.0.113.7 192.0.2.10 51234 25565\r\n".to_vec();
        let mut long_unknown = b"PROXY UNKNOWN ".to_vec();
        long_unknown.extend([b'a'; 91]);
        long_unknown.extend(b"\r\n");
        let cases: [(Vec<u8>, Option<SocketAddr>); 9] = [
            (v1_tcp4.clone(), client("203.0.113.7:51234")),
            (
                b"PROXY TCP6 2001:db8::7 2001:db8::1 51234 25565\r\n".to_vec(),
                client("[2001:db8::7]:51234"),
            ),
            (
                b"PROXY TCP6 ::ffff:203.0.113.7 ::ffff:192.0.2.10 51234 25565\r\n".to_vec(),
                client("203.0.113.7:51234"),
            ),
            (b"PROXY UNKNOWN\r\n".to_vec(), None),
            (b"PROXY UNKNOWN ffff:f:: ffff:f:: 1 2\r\n".to_vec(), None),
            (b"PROXY UNKNOWN a  b\r\n".to_vec(), None),
            (long_unknown, None),
            (hex(BLOCK_A), client("203.0.113.7:51234")),
            (hex(BLOCK_B), None),
        ];
        for (case, (header, expected)) in cases.into_iter().enumerate() {
            let mut input = header.clone();
            input.extend([0x10, 0x00, 0xff]);
            let (result, consumed) = read(&input);
            assert_eq!(result.unwrap(), expected, "case {case}");
            assert_eq!(consumed, header.len(), "case {case}");
        }
    }

    #[test]
    fn a_header_sent_one_byte_at_a_time_is_read() {
        let header = b"PROXY TCP4 203.0.113.7 192.0.2.10 51234 25565\r\n";
        let mut stream = one_byte_at_a_time(header);
        assert_eq!(
            block_on(read_proxy_header(&mut stream)).unwrap(),
            client("203.0.113.7:51234")
        );
    }

    fn one_byte_at_a_time(bytes: &'static [u8]) -> impl AsyncRead + Unpin {
        struct OneByte(&'static [u8]);
        impl AsyncRead for OneByte {
            fn poll_read(
                mut self: std::pin::Pin<&mut Self>,
                _: &mut std::task::Context<'_>,
                buf: &mut tokio::io::ReadBuf<'_>,
            ) -> std::task::Poll<Result<()>> {
                if let Some((first, rest)) = self.0.split_first() {
                    buf.put_slice(&[*first]);
                    self.0 = rest;
                }
                std::task::Poll::Ready(Ok(()))
            }
        }
        OneByte(bytes)
    }

    #[test]
    fn malformed_headers_are_refused() {
        let mut too_long = b"PROXY UNKNOWN ".to_vec();
        too_long.extend([b'a'; 92]);
        too_long.extend(b"\r\n");
        let mut wrong_signature = hex(BLOCK_A);
        wrong_signature[11] = 0x0b;
        let cases: [Vec<u8>; 16] = [
            b"PROXZ TCP4 203.0.113.7 192.0.2.10 51234 25565\r\n".to_vec(),
            b"proxy TCP4 203.0.113.7 192.0.2.10 51234 25565\r\n".to_vec(),
            wrong_signature,
            too_long,
            b"PROXY TCP4 203.0.113.07 192.0.2.10 51234 25565\r\n".to_vec(),
            b"PROXY TCP4 203.0.113.7 192.0.2.10 51234 025565\r\n".to_vec(),
            b"PROXY TCP4 203.0.113.7  192.0.2.10 51234 25565\r\n".to_vec(),
            b"PROXY TCP4 203.0.113.7 192.0.2.10 51234 25565\n".to_vec(),
            b"PROXY TCP6 203.0.113.7 192.0.2.10 51234 25565\r\n".to_vec(),
            b"PROXY TCP4 203.0.113.7 192.0.2.10 +1 25565\r\n".to_vec(),
            b"PROXY TCP4 203.0.113.7 192.0.2.10 51234 65536\r\n".to_vec(),
            b"PROXY TCP6 ::ffff:203.0.113.07 2001:db8::1 51234 25565\r\n".to_vec(),
            b"PROXY TCP4 203.0.113.7 192.0.2.10 51234 25565 \r\n".to_vec(),
            hex(BLOCK_C),
            hex(BLOCK_D),
            b"\x10\x00\xff\x05\x09localhost\x63\xdd\x02".to_vec(),
        ];
        for (case, input) in cases.into_iter().enumerate() {
            let (result, consumed) = read(&input);
            assert!(result.is_err(), "case {case}: {result:?}");
            assert!(consumed <= V1_MAX_LEN, "case {case}");
        }
    }
}
