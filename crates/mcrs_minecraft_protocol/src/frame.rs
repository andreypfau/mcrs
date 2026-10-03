use bytes::{Buf, BufMut, Bytes, BytesMut};
use flate2::{Compress, Compression, Decompress, FlushCompress, FlushDecompress, Status};
use thiserror::Error;

use crate::CompressionThreshold;
use crate::Decode;
use crate::var_int::VarInt;

pub const MAX_FRAME_BODY: usize = 2_097_151;

pub const MAX_UNCOMPRESSED_PACKET: usize = 8_388_608;

const MAX_LENGTH_BYTES: usize = 3;

const MAX_DATA_LENGTH_BYTES: usize = 4;

const MIN_INFLATE_STEP: usize = 4096;

const MIN_DEFLATE_STEP: usize = 4096;

const COMPRESSION_LEVEL: u32 = 4;

#[derive(Copy, Clone, PartialEq, Eq, Debug, Error)]
pub enum FrameError {
    #[error("incomplete frame length")]
    IncompleteLength,
    #[error("frame length is wider than three bytes")]
    MalformedLength,
    #[error("frame length is zero")]
    ZeroLength,
    #[error("frame body is {missing} bytes short")]
    ShortBody { missing: usize },
    #[error("malformed data length")]
    MalformedDataLength,
    #[error("compressed packet of {declared} bytes is below the threshold of {threshold}")]
    BelowThreshold { declared: usize, threshold: usize },
    #[error("compressed packet of {declared} bytes is above the maximum")]
    AboveMaximum { declared: usize },
    #[error("compressed packet declared {declared} bytes but holds {inflated}")]
    ShortStream { declared: usize, inflated: usize },
    #[error("compressed packet holds more than it declared")]
    TrailingData,
    #[error("corrupt compressed stream")]
    CorruptStream,
    #[error("a packet must hold at least its id")]
    EmptyPacket,
    #[error("packet of {len} bytes is above the maximum")]
    PacketTooLarge { len: usize },
    #[error("frame of {len} bytes is wider than its length can state")]
    FrameTooLarge { len: usize },
    #[error("the compressor refused the packet")]
    CompressionFailed,
}

impl FrameError {
    pub fn is_incomplete(&self) -> bool {
        matches!(self, Self::IncompleteLength | Self::ShortBody { .. })
    }
}

pub struct Deflate {
    compress: Compress,
    // chisle: keeps the capacity of the largest packet compressed so far; release it if that ever shows up in memory use
    scratch: Vec<u8>,
}

impl Deflate {
    fn new() -> Self {
        Self {
            compress: Compress::new(Compression::new(COMPRESSION_LEVEL), true),
            scratch: Vec::new(),
        }
    }

    fn deflate(&mut self, packet: &[u8]) -> Result<(), FrameError> {
        self.scratch.clear();
        let result = deflate_stream(&mut self.compress, packet, &mut self.scratch);
        self.compress.reset();
        result
    }
}

fn deflate_stream(
    compress: &mut Compress,
    packet: &[u8],
    out: &mut Vec<u8>,
) -> Result<(), FrameError> {
    let start_in = compress.total_in();
    loop {
        if out.capacity() - out.len() < MIN_DEFLATE_STEP {
            out.reserve(out.len().max(MIN_DEFLATE_STEP));
        }
        let read = (compress.total_in() - start_in) as usize;
        let out_before = compress.total_out();
        let status = compress
            .compress_vec(&packet[read..], out, FlushCompress::Finish)
            .map_err(|_| FrameError::CompressionFailed)?;
        if status == Status::StreamEnd {
            return Ok(());
        }
        let progressed =
            compress.total_out() != out_before || (compress.total_in() - start_in) as usize != read;
        if !progressed {
            return Err(FrameError::CompressionFailed);
        }
    }
}

pub fn encode_frame(
    buf: &mut BytesMut,
    start: usize,
    threshold: CompressionThreshold,
    deflate: &mut Option<Deflate>,
) -> Result<(), FrameError> {
    let result = frame_in_place(buf, start, threshold, deflate);
    if result.is_err() {
        buf.truncate(start);
    }
    result
}

fn frame_in_place(
    buf: &mut BytesMut,
    start: usize,
    threshold: CompressionThreshold,
    deflate: &mut Option<Deflate>,
) -> Result<(), FrameError> {
    let len = buf.len() - start;
    if len == 0 {
        return Err(FrameError::EmptyPacket);
    }

    let Ok(threshold) = usize::try_from(threshold.0) else {
        check_body(len)?;
        insert_before_packet(buf, start, body_prefix(len, None));
        return Ok(());
    };

    if len < threshold {
        check_body(len + 1)?;
        insert_before_packet(buf, start, body_prefix(len + 1, Some(0)));
        return Ok(());
    }

    if len > MAX_UNCOMPRESSED_PACKET {
        return Err(FrameError::PacketTooLarge { len });
    }
    let deflate = deflate.get_or_insert_with(Deflate::new);
    deflate.deflate(&buf[start..])?;
    let body = VarInt(len as i32).written_size() + deflate.scratch.len();
    check_body(body)?;
    buf.truncate(start);
    let (prefix, width) = body_prefix(body, Some(len));
    buf.extend_from_slice(&prefix[..width]);
    buf.extend_from_slice(&deflate.scratch);
    Ok(())
}

fn check_body(len: usize) -> Result<(), FrameError> {
    if len > MAX_FRAME_BODY {
        return Err(FrameError::FrameTooLarge { len });
    }
    Ok(())
}

type Prefix = ([u8; MAX_LENGTH_BYTES + MAX_DATA_LENGTH_BYTES], usize);

fn body_prefix(body: usize, data_length: Option<usize>) -> Prefix {
    let mut prefix = [0u8; MAX_LENGTH_BYTES + MAX_DATA_LENGTH_BYTES];
    let mut width = put_var_int(&mut prefix, 0, body);
    if let Some(data_length) = data_length {
        width = put_var_int(&mut prefix, width, data_length);
    }
    (prefix, width)
}

fn put_var_int(dst: &mut [u8], mut at: usize, mut value: usize) -> usize {
    loop {
        let low = (value & 0x7f) as u8;
        value >>= 7;
        if value == 0 {
            dst[at] = low;
            return at + 1;
        }
        dst[at] = low | 0x80;
        at += 1;
    }
}

fn insert_before_packet(buf: &mut BytesMut, start: usize, (prefix, width): Prefix) {
    let end = buf.len();
    buf.put_bytes(0, width);
    buf.copy_within(start..end, start + width);
    buf[start..start + width].copy_from_slice(&prefix[..width]);
}

pub fn split_frame(buf: &mut BytesMut) -> Result<Bytes, FrameError> {
    let mut length = 0usize;
    let mut width = 0;
    loop {
        let byte = *buf.get(width).ok_or(FrameError::IncompleteLength)?;
        length |= usize::from(byte & 0x7f) << (7 * width);
        width += 1;
        if byte & 0x80 == 0 {
            break;
        }
        if width == MAX_LENGTH_BYTES {
            return Err(FrameError::MalformedLength);
        }
    }
    if length == 0 {
        return Err(FrameError::ZeroLength);
    }
    let available = buf.len() - width;
    if available < length {
        return Err(FrameError::ShortBody {
            missing: length - available,
        });
    }
    buf.advance(width);
    Ok(buf.split_to(length).freeze())
}

pub fn decompress(
    frame: Bytes,
    threshold: CompressionThreshold,
    inflater: &mut Option<Decompress>,
) -> Result<Bytes, FrameError> {
    let Ok(threshold) = usize::try_from(threshold.0) else {
        return Ok(frame);
    };

    let mut rest = &frame[..];
    let declared = VarInt::decode(&mut rest).map_err(|_| FrameError::MalformedDataLength)?;
    let declared = usize::try_from(declared.0).map_err(|_| FrameError::MalformedDataLength)?;
    let width = frame.len() - rest.len();

    if declared == 0 {
        return Ok(frame.slice(width..));
    }
    if declared < threshold {
        return Err(FrameError::BelowThreshold {
            declared,
            threshold,
        });
    }
    if declared > MAX_UNCOMPRESSED_PACKET {
        return Err(FrameError::AboveMaximum { declared });
    }

    let inflater = inflater.get_or_insert_with(|| Decompress::new(true));
    let mut out = Vec::new();
    inflate(rest, declared, inflater, &mut out)?;
    Ok(Bytes::from(out))
}

fn inflate(
    input: &[u8],
    declared: usize,
    inflater: &mut Decompress,
    out: &mut Vec<u8>,
) -> Result<(), FrameError> {
    let start_in = inflater.total_in();
    let result = inflate_stream(input, declared, inflater, start_in, out);
    inflater.reset(true);
    result
}

fn inflate_stream(
    input: &[u8],
    declared: usize,
    inflater: &mut Decompress,
    start_in: u64,
    out: &mut Vec<u8>,
) -> Result<(), FrameError> {
    let step = input.len().saturating_mul(4).max(MIN_INFLATE_STEP);
    let consumed = |inflater: &Decompress| (inflater.total_in() - start_in) as usize;

    loop {
        let len = out.len();
        let remaining = declared - len;
        if remaining == 0 {
            return check_stream_end(&input[consumed(inflater)..], inflater);
        }

        let chunk = remaining.min(step.max(len));
        out.reserve_exact(chunk);
        out.resize(len + chunk, 0);

        let read = consumed(inflater);
        let out_before = inflater.total_out();
        let status = inflater.decompress(&input[read..], &mut out[len..], FlushDecompress::None);
        let produced = (inflater.total_out() - out_before) as usize;
        out.truncate(len + produced);
        let status = status.map_err(|_| FrameError::CorruptStream)?;

        let read_after = consumed(inflater);
        if status == Status::StreamEnd {
            return if out.len() < declared {
                Err(FrameError::ShortStream {
                    declared,
                    inflated: out.len(),
                })
            } else if read_after < input.len() {
                Err(FrameError::TrailingData)
            } else {
                Ok(())
            };
        }
        if produced == 0 && read_after == read {
            return Err(FrameError::CorruptStream);
        }
    }
}

fn check_stream_end(unread: &[u8], inflater: &mut Decompress) -> Result<(), FrameError> {
    let in_before = inflater.total_in();
    let out_before = inflater.total_out();
    let mut probe = [0u8; 1];
    let status = inflater
        .decompress(unread, &mut probe, FlushDecompress::None)
        .map_err(|_| FrameError::CorruptStream)?;
    let read = (inflater.total_in() - in_before) as usize;
    let produced = inflater.total_out() != out_before;

    match status {
        Status::StreamEnd if !produced && read == unread.len() => Ok(()),
        _ if produced || read < unread.len() => Err(FrameError::TrailingData),
        _ => Err(FrameError::CorruptStream),
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use flate2::Compression;
    use flate2::write::ZlibEncoder;
    use rand::rngs::StdRng;
    use rand::{RngExt, SeedableRng};

    use super::*;
    use crate::Encode;
    use crate::var_int::VarInt;

    fn deflate(data: &[u8]) -> Vec<u8> {
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(data).unwrap();
        encoder.finish().unwrap()
    }

    fn var_int(value: i32) -> Vec<u8> {
        let mut out = Vec::new();
        VarInt(value).encode(&mut out).unwrap();
        out
    }

    fn pattern(len: usize) -> Vec<u8> {
        (0..len).map(|i| (i * 7 % 251) as u8).collect()
    }

    fn compressed_frame(declared: i32, stream: &[u8]) -> Bytes {
        let mut frame = var_int(declared);
        frame.extend_from_slice(stream);
        Bytes::from(frame)
    }

    fn uncompressed_frame(rest: &[u8]) -> Bytes {
        let mut frame = vec![0x00];
        frame.extend_from_slice(rest);
        Bytes::from(frame)
    }

    fn framed(body: &[u8]) -> BytesMut {
        let mut buf = BytesMut::from(&var_int(body.len() as i32)[..]);
        buf.extend_from_slice(body);
        buf
    }

    #[test]
    fn an_empty_buffer_is_an_incomplete_length() {
        let mut buf = BytesMut::new();
        assert_eq!(split_frame(&mut buf), Err(FrameError::IncompleteLength));
    }

    #[test]
    fn one_or_two_continuation_bytes_are_an_incomplete_length() {
        for prefix in [&[0x80][..], &[0x80, 0x80][..]] {
            let mut buf = BytesMut::from(prefix);
            assert_eq!(split_frame(&mut buf), Err(FrameError::IncompleteLength));
        }
    }

    #[test]
    fn a_third_continuation_byte_is_a_malformed_length() {
        let mut buf = BytesMut::from(&[0x80, 0x80, 0x80][..]);
        assert_eq!(split_frame(&mut buf), Err(FrameError::MalformedLength));
        let mut buf = BytesMut::from(&[0xFF, 0xFF, 0xFF, 0xFF, 0x0F, 1][..]);
        assert_eq!(split_frame(&mut buf), Err(FrameError::MalformedLength));
    }

    #[test]
    fn a_zero_length_is_refused() {
        let mut buf = BytesMut::from(&[0x00][..]);
        assert_eq!(split_frame(&mut buf), Err(FrameError::ZeroLength));
        let mut buf = BytesMut::from(&[0x80, 0x80, 0x00, 9][..]);
        assert_eq!(split_frame(&mut buf), Err(FrameError::ZeroLength));
    }

    #[test]
    fn a_body_shorter_than_its_length_reports_how_much_is_missing() {
        let mut buf = BytesMut::from(&[5, 1, 2][..]);
        assert_eq!(
            split_frame(&mut buf),
            Err(FrameError::ShortBody { missing: 3 })
        );
        let mut buf = BytesMut::from(&[5][..]);
        assert_eq!(
            split_frame(&mut buf),
            Err(FrameError::ShortBody { missing: 5 })
        );
    }

    #[test]
    fn a_frame_of_one_byte_is_returned() {
        let mut buf = BytesMut::from(&[1, 0x2A][..]);
        assert_eq!(split_frame(&mut buf).unwrap(), Bytes::from_static(&[0x2A]));
        assert!(buf.is_empty());
    }

    #[test]
    fn a_padded_length_frames_by_the_bytes_read() {
        let mut buf = BytesMut::from(&[0x81, 0x80, 0x00, 0x2A][..]);
        assert_eq!(split_frame(&mut buf).unwrap(), Bytes::from_static(&[0x2A]));
        assert!(buf.is_empty());
    }

    #[test]
    fn the_buffer_is_untouched_unless_a_frame_is_returned() {
        let cases: [&[u8]; 6] = [
            &[],
            &[0x80],
            &[0x80, 0x80],
            &[0x80, 0x80, 0x80, 7],
            &[0x00, 7],
            &[5, 1, 2],
        ];
        for case in cases {
            let mut buf = BytesMut::from(case);
            assert!(split_frame(&mut buf).is_err(), "{case:?}");
            assert_eq!(&buf[..], case);
        }
    }

    #[test]
    fn two_frames_in_one_buffer_split_one_per_call() {
        let mut buf = framed(&[1, 2, 3]);
        let second = framed(&[4, 5]);
        buf.extend_from_slice(&second);

        assert_eq!(
            split_frame(&mut buf).unwrap(),
            Bytes::from_static(&[1, 2, 3])
        );
        assert_eq!(buf, second);
        assert_eq!(split_frame(&mut buf).unwrap(), Bytes::from_static(&[4, 5]));
        assert!(buf.is_empty());
    }

    #[test]
    fn a_frame_of_the_largest_length_is_returned() {
        let body = vec![0xA5; MAX_FRAME_BODY];
        let mut buf = BytesMut::from(&[0xFF, 0xFF, 0x7F][..]);
        buf.extend_from_slice(&body);
        let frame = split_frame(&mut buf).unwrap();
        assert_eq!(frame.len(), MAX_FRAME_BODY);
        assert_eq!(&frame[..], &body[..]);
        assert!(buf.is_empty());
    }

    #[test]
    fn a_negative_threshold_returns_the_frame_unchanged() {
        let mut inflater = None;
        for frame in [vec![1, 2, 3, 4], vec![0x80], vec![0x00, 9], vec![]] {
            let frame = Bytes::from(frame);
            assert_eq!(
                decompress(frame.clone(), CompressionThreshold(-1), &mut inflater),
                Ok(frame)
            );
        }
        assert!(inflater.is_none());
    }

    #[test]
    fn a_declared_size_at_the_maximum_is_accepted_and_one_past_it_refused() {
        let threshold = CompressionThreshold(256);
        let two_megabytes = vec![0u8; 2_097_152];
        let frame = compressed_frame(2_097_152, &deflate(&two_megabytes));
        assert_eq!(
            decompress(frame, threshold, &mut None),
            Ok(Bytes::from(two_megabytes))
        );

        let at_maximum = vec![0u8; MAX_UNCOMPRESSED_PACKET];
        let frame = compressed_frame(MAX_UNCOMPRESSED_PACKET as i32, &deflate(&at_maximum));
        let inflated = decompress(frame, threshold, &mut None).unwrap();
        assert_eq!(inflated.len(), MAX_UNCOMPRESSED_PACKET);

        let mut inflater = None;
        let frame = compressed_frame(MAX_UNCOMPRESSED_PACKET as i32 + 1, &deflate(&[0; 16]));
        assert_eq!(
            decompress(frame, threshold, &mut inflater),
            Err(FrameError::AboveMaximum {
                declared: MAX_UNCOMPRESSED_PACKET + 1
            })
        );
        assert!(inflater.is_none());
    }

    #[test]
    fn a_malformed_or_oversized_declared_size_is_refused_before_inflating() {
        let stream = deflate(&[0; 16]);
        let cases = [
            (
                compressed_frame(-1, &stream),
                FrameError::MalformedDataLength,
            ),
            (Bytes::from_static(&[0x80]), FrameError::MalformedDataLength),
            (Bytes::new(), FrameError::MalformedDataLength),
            (
                Bytes::from_static(&[0xFF; 6]),
                FrameError::MalformedDataLength,
            ),
            (
                compressed_frame(i32::MAX, &stream),
                FrameError::AboveMaximum {
                    declared: i32::MAX as usize,
                },
            ),
        ];
        for (frame, error) in cases {
            let mut inflater = None;
            assert_eq!(
                decompress(frame, CompressionThreshold(64), &mut inflater),
                Err(error)
            );
            assert!(inflater.is_none(), "{error:?} created an inflater");
        }
    }

    #[test]
    fn a_padded_data_length_is_read_by_its_bytes() {
        let threshold = CompressionThreshold(1);
        let mut inflater = None;
        for padding in [&[0x80, 0x00][..], &[0x80, 0x80, 0x00][..]] {
            let mut frame = padding.to_vec();
            frame.extend_from_slice(&[9, 9, 9]);
            assert_eq!(
                decompress(Bytes::from(frame), threshold, &mut inflater),
                Ok(Bytes::from_static(&[9, 9, 9]))
            );
        }

        let data = pattern(10);
        let mut frame = vec![0x8A, 0x00];
        frame.extend_from_slice(&deflate(&data));
        assert_eq!(
            decompress(Bytes::from(frame), threshold, &mut inflater),
            Ok(Bytes::from(data))
        );
    }

    #[test]
    fn a_stream_shorter_than_declared_is_refused() {
        let frame = compressed_frame(20, &deflate(&pattern(10)));
        assert_eq!(
            decompress(frame, CompressionThreshold(1), &mut None),
            Err(FrameError::ShortStream {
                declared: 20,
                inflated: 10
            })
        );
    }

    #[test]
    fn a_stream_longer_than_declared_is_refused() {
        let frame = compressed_frame(10, &deflate(&pattern(20)));
        assert_eq!(
            decompress(frame, CompressionThreshold(1), &mut None),
            Err(FrameError::TrailingData)
        );
    }

    #[test]
    fn bytes_after_the_end_of_the_stream_are_refused() {
        let mut stream = deflate(&pattern(10));
        stream.push(0);
        assert_eq!(
            decompress(
                compressed_frame(10, &stream),
                CompressionThreshold(1),
                &mut None
            ),
            Err(FrameError::TrailingData)
        );
    }

    #[test]
    fn a_corrupt_stream_is_refused() {
        let frame = compressed_frame(10, &[0xDE, 0xAD, 0xBE, 0xEF, 0x00, 0x01]);
        assert_eq!(
            decompress(frame, CompressionThreshold(1), &mut None),
            Err(FrameError::CorruptStream)
        );
    }

    #[test]
    fn a_truncated_stream_is_refused() {
        let stream = deflate(&pattern(10));
        for cut in [1, 2, 4] {
            let frame = compressed_frame(10, &stream[..stream.len() - cut]);
            assert_eq!(
                decompress(frame, CompressionThreshold(1), &mut None),
                Err(FrameError::CorruptStream),
                "cut {cut}"
            );
        }
    }

    #[test]
    fn the_inflater_is_usable_after_a_refused_frame() {
        let threshold = CompressionThreshold(1);
        let mut truncated = deflate(&pattern(10));
        truncated.truncate(truncated.len() - 2);
        let mut trailing = deflate(&pattern(10));
        trailing.push(0);
        let refused = [
            compressed_frame(20, &deflate(&pattern(10))),
            compressed_frame(10, &deflate(&pattern(20))),
            compressed_frame(10, &trailing),
            compressed_frame(10, &[0xDE, 0xAD, 0xBE, 0xEF, 0x00, 0x01]),
            compressed_frame(10, &truncated),
        ];

        let mut inflater = None;
        for frame in refused {
            assert!(decompress(frame, threshold, &mut inflater).is_err());
            let data = pattern(300);
            let valid = compressed_frame(300, &deflate(&data));
            assert_eq!(
                decompress(valid, threshold, &mut inflater),
                Ok(Bytes::from(data))
            );
        }
        assert!(inflater.is_some());
    }

    #[test]
    fn a_small_frame_declaring_the_maximum_does_not_reserve_it() {
        let input = deflate(&pattern(10));
        let mut inflater = Decompress::new(true);
        let mut out = Vec::new();
        assert_eq!(
            inflate(&input, MAX_UNCOMPRESSED_PACKET, &mut inflater, &mut out),
            Err(FrameError::ShortStream {
                declared: MAX_UNCOMPRESSED_PACKET,
                inflated: 10
            })
        );
        assert!(out.capacity() < 1 << 20, "capacity {}", out.capacity());
    }

    #[test]
    fn an_empty_rest_is_returned_as_an_empty_packet() {
        assert_eq!(
            decompress(
                Bytes::from_static(&[0x00]),
                CompressionThreshold(64),
                &mut None
            ),
            Ok(Bytes::new())
        );
    }

    fn written(packet: &[u8], threshold: i32) -> BytesMut {
        let mut buf = BytesMut::from(packet);
        encode_frame(&mut buf, 0, CompressionThreshold(threshold), &mut None).unwrap();
        buf
    }

    fn read_back(buf: &mut BytesMut, threshold: i32) -> Result<Bytes, FrameError> {
        let frame = split_frame(buf)?;
        decompress(frame, CompressionThreshold(threshold), &mut None)
    }

    fn data_length(frame: &Bytes) -> usize {
        let mut rest = &frame[..];
        VarInt::decode(&mut rest).unwrap().0 as usize
    }

    #[test]
    fn a_packet_becomes_a_frame_and_reads_back_with_compression_off() {
        let packet = pattern(300);
        let mut buf = written(&packet, -1);
        assert_eq!(&buf[..2], &[0xAC, 0x02]);
        assert_eq!(&buf[2..], &packet[..]);
        assert_eq!(read_back(&mut buf, -1), Ok(Bytes::from(packet)));
        assert!(buf.is_empty());
    }

    #[test]
    fn a_frame_is_appended_after_what_the_buffer_already_holds() {
        let first = pattern(40);
        let second = pattern(400);
        let mut buf = BytesMut::new();
        let mut deflate = None;
        for packet in [&first, &second] {
            let start = buf.len();
            buf.extend_from_slice(packet);
            encode_frame(&mut buf, start, CompressionThreshold(256), &mut deflate).unwrap();
        }
        assert_eq!(read_back(&mut buf, 256), Ok(Bytes::from(first)));
        assert_eq!(read_back(&mut buf, 256), Ok(Bytes::from(second)));
        assert!(buf.is_empty());
    }

    #[derive(Clone, Copy, Debug)]
    enum Direction {
        Write,
        ReadCompressed,
        ReadUncompressed,
    }

    #[derive(Clone, Copy, Debug)]
    enum Outcome {
        Uncompressed,
        Compressed,
        Accepted,
        Refused(FrameError),
    }

    #[test]
    fn every_rule_holds_at_its_boundary_in_both_directions() {
        use Direction::*;
        use Outcome::*;

        let below = |declared, threshold| {
            Refused(FrameError::BelowThreshold {
                declared,
                threshold,
            })
        };
        let rows = [
            (Write, 1, 0, Refused(FrameError::EmptyPacket)),
            (Write, 1, 1, Compressed),
            (Write, 1, 2, Compressed),
            (ReadCompressed, 1, 0, Uncompressed),
            (ReadCompressed, 1, 1, Accepted),
            (ReadCompressed, 1, 2, Accepted),
            (ReadUncompressed, 1, 0, Accepted),
            (ReadUncompressed, 1, 1, Accepted),
            (ReadUncompressed, 1, 2, Accepted),
            (Write, 64, 63, Uncompressed),
            (Write, 64, 64, Compressed),
            (Write, 64, 65, Compressed),
            (ReadCompressed, 64, 63, below(63, 64)),
            (ReadCompressed, 64, 64, Accepted),
            (ReadCompressed, 64, 65, Accepted),
            (ReadUncompressed, 64, 63, Accepted),
            (ReadUncompressed, 64, 64, Accepted),
            (ReadUncompressed, 64, 65, Accepted),
            (Write, 256, 255, Uncompressed),
            (Write, 256, 256, Compressed),
            (Write, 256, 257, Compressed),
            (ReadCompressed, 256, 255, below(255, 256)),
            (ReadCompressed, 256, 256, Accepted),
            (ReadCompressed, 256, 257, Accepted),
            (ReadUncompressed, 256, 255, Accepted),
            (ReadUncompressed, 256, 256, Accepted),
            (ReadUncompressed, 256, 257, Accepted),
            (ReadUncompressed, 1, 100_000, Accepted),
            (ReadUncompressed, 64, 100_000, Accepted),
            (ReadUncompressed, 256, 100_000, Accepted),
        ];

        let mut ran = 0;
        for (direction, threshold, size, outcome) in rows {
            let context = format!("{direction:?} threshold {threshold} size {size}");
            let packet = pattern(size);
            match (direction, outcome) {
                (Write, Refused(error)) => {
                    let mut buf = BytesMut::from(&packet[..]);
                    let result =
                        encode_frame(&mut buf, 0, CompressionThreshold(threshold), &mut None);
                    assert_eq!(result, Err(error), "{context}");
                    assert!(buf.is_empty(), "{context}");
                }
                (Write, Uncompressed | Compressed) => {
                    let mut buf = written(&packet, threshold);
                    let frame = split_frame(&mut buf.clone()).unwrap();
                    let expected = if matches!(outcome, Compressed) {
                        size
                    } else {
                        0
                    };
                    assert_eq!(data_length(&frame), expected, "{context}");
                    if matches!(outcome, Compressed) {
                        let width = VarInt(size as i32).written_size();
                        assert_eq!(frame[width], 0x78, "{context}");
                    }
                    assert_eq!(
                        read_back(&mut buf, threshold),
                        Ok(Bytes::from(packet)),
                        "{context}"
                    );
                    assert!(buf.is_empty(), "{context}");
                }
                (ReadCompressed, _) => {
                    let stream = deflate(&packet);
                    let frame = compressed_frame(size as i32, &stream);
                    let expected = match outcome {
                        Accepted => Ok(Bytes::from(packet)),
                        Uncompressed => Ok(Bytes::from(stream)),
                        Refused(error) => Err(error),
                        Compressed => unreachable!("{context}"),
                    };
                    let result = decompress(frame, CompressionThreshold(threshold), &mut None);
                    assert_eq!(result, expected, "{context}");
                }
                (ReadUncompressed, Accepted) => {
                    let mut inflater = None;
                    let result = decompress(
                        uncompressed_frame(&packet),
                        CompressionThreshold(threshold),
                        &mut inflater,
                    );
                    assert_eq!(result, Ok(Bytes::from(packet)), "{context}");
                    assert!(inflater.is_none(), "{context}");
                }
                _ => unreachable!("{context}"),
            }
            ran += 1;
        }
        assert_eq!(ran, 30);
        assert_eq!(rows.len(), 30);
    }

    fn incompressible(len: usize) -> Vec<u8> {
        let mut bytes = vec![0u8; len];
        StdRng::seed_from_u64(0x9E37_79B9_7F4A_7C15).fill(&mut bytes[..]);
        bytes
    }

    #[test]
    fn the_frame_and_packet_limits_hold_on_write() {
        let off = -1;
        let largest_body = vec![0xA5; MAX_FRAME_BODY];
        let mut buf = written(&largest_body, off);
        assert_eq!(&buf[..3], &[0xFF, 0xFF, 0x7F]);
        assert_eq!(read_back(&mut buf, off), Ok(Bytes::from(largest_body)));

        let mut buf = BytesMut::from(&vec![0xA5; MAX_FRAME_BODY + 1][..]);
        assert_eq!(
            encode_frame(&mut buf, 0, CompressionThreshold(off), &mut None),
            Err(FrameError::FrameTooLarge {
                len: MAX_FRAME_BODY + 1
            })
        );

        let on = 256;
        let at_maximum = vec![0u8; MAX_UNCOMPRESSED_PACKET];
        let mut buf = written(&at_maximum, on);
        let inflated = read_back(&mut buf, on).unwrap();
        assert_eq!(inflated.len(), MAX_UNCOMPRESSED_PACKET);
        assert!(inflated.iter().all(|&byte| byte == 0));

        let mut buf = BytesMut::from(&vec![0u8; MAX_UNCOMPRESSED_PACKET + 1][..]);
        assert_eq!(
            encode_frame(&mut buf, 0, CompressionThreshold(on), &mut None),
            Err(FrameError::PacketTooLarge {
                len: MAX_UNCOMPRESSED_PACKET + 1
            })
        );

        let above_every_packet = 3_000_000;
        let fits = vec![0xA5; MAX_FRAME_BODY - 1];
        let mut buf = written(&fits, above_every_packet);
        assert_eq!(
            read_back(&mut buf, above_every_packet),
            Ok(Bytes::from(fits))
        );

        let too_long_by_the_data_length = vec![0xA5; MAX_FRAME_BODY];
        let mut buf = BytesMut::from(&too_long_by_the_data_length[..]);
        assert_eq!(
            encode_frame(
                &mut buf,
                0,
                CompressionThreshold(above_every_packet),
                &mut None
            ),
            Err(FrameError::FrameTooLarge {
                len: MAX_FRAME_BODY + 1
            })
        );

        let mut buf = BytesMut::from(&incompressible(MAX_FRAME_BODY + 1)[..]);
        assert!(matches!(
            encode_frame(&mut buf, 0, CompressionThreshold(on), &mut None),
            Err(FrameError::FrameTooLarge { len }) if len > MAX_FRAME_BODY
        ));
    }

    #[test]
    fn a_refused_packet_leaves_nothing_in_the_buffer() {
        let before = [9u8, 8, 7];
        let cases = [
            (-1, vec![0xA5; MAX_FRAME_BODY + 1]),
            (3_000_000, vec![0xA5; MAX_FRAME_BODY]),
            (256, vec![0u8; MAX_UNCOMPRESSED_PACKET + 1]),
            (256, incompressible(MAX_FRAME_BODY + 1)),
            (-1, Vec::new()),
            (0, Vec::new()),
            (1, Vec::new()),
        ];
        for (threshold, packet) in cases {
            let mut deflate = None;
            let mut buf = BytesMut::from(&before[..]);
            buf.extend_from_slice(&packet);
            let result = encode_frame(&mut buf, 3, CompressionThreshold(threshold), &mut deflate);
            if packet.is_empty() {
                assert_eq!(
                    result,
                    Err(FrameError::EmptyPacket),
                    "threshold {threshold}"
                );
            }
            assert!(
                result.is_err(),
                "threshold {threshold}, {} bytes",
                packet.len()
            );
            assert_eq!(
                &buf[..],
                &before[..],
                "threshold {threshold}, {} bytes",
                packet.len()
            );

            let next = pattern(300);
            buf.extend_from_slice(&next);
            encode_frame(&mut buf, 3, CompressionThreshold(threshold), &mut deflate).unwrap();
            assert_eq!(&buf[..3], &before[..]);
            let mut frame = buf.split_off(3);
            assert_eq!(read_back(&mut frame, threshold), Ok(Bytes::from(next)));
        }
    }

    #[test]
    fn the_compressor_is_created_on_first_use_and_reused() {
        let mut deflate = None;
        for (threshold, size) in [(-1, 300), (256, 255)] {
            let mut buf = BytesMut::from(&pattern(size)[..]);
            encode_frame(&mut buf, 0, CompressionThreshold(threshold), &mut deflate).unwrap();
            assert!(deflate.is_none(), "threshold {threshold}, size {size}");
        }

        let first = incompressible(20_000);
        let mut buf = BytesMut::from(&first[..]);
        encode_frame(&mut buf, 0, CompressionThreshold(256), &mut deflate).unwrap();
        let capacity = deflate.as_ref().unwrap().scratch.capacity();
        assert!(capacity > MIN_DEFLATE_STEP);
        assert_eq!(read_back(&mut buf, 256), Ok(Bytes::from(first)));

        let second = pattern(700);
        let mut reused = BytesMut::from(&second[..]);
        encode_frame(&mut reused, 0, CompressionThreshold(256), &mut deflate).unwrap();
        assert_eq!(deflate.as_ref().unwrap().scratch.capacity(), capacity);
        assert_eq!(reused, written(&second, 256));
        assert_eq!(read_back(&mut reused, 256), Ok(Bytes::from(second)));
    }

    fn prefix_width(frame: &[u8]) -> usize {
        frame.iter().position(|byte| byte & 0x80 == 0).unwrap() + 1
    }

    #[test]
    fn the_length_prefix_is_as_short_as_its_value_allows() {
        let bodies = [
            (127, 1),
            (128, 2),
            (16_383, 2),
            (16_384, 3),
            (MAX_FRAME_BODY, 3),
        ];
        for (body, width) in bodies {
            let off = written(&pattern(body), -1);
            assert_eq!(prefix_width(&off), width, "off, body {body}");
            assert_eq!(off.len(), body + width, "off, body {body}");

            let uncompressed_by_threshold = written(&pattern(body - 1), 3_000_000);
            assert_eq!(
                prefix_width(&uncompressed_by_threshold),
                width,
                "below the threshold, body {body}"
            );
            assert_eq!(
                uncompressed_by_threshold.len(),
                body + width,
                "below the threshold, body {body}"
            );
        }
    }

    const SWITCH_AFTER_FRAME: usize = 1;

    fn five_frame_stream() -> (Vec<u8>, Vec<Bytes>) {
        let sizes_and_thresholds = [(20, -1), (60, -1), (100, 256), (400, 256), (1000, 256)];
        let mut stream = BytesMut::new();
        let mut deflate = None;
        let mut packets = Vec::new();
        for (index, (size, threshold)) in sizes_and_thresholds.into_iter().enumerate() {
            let mut packet = pattern(size);
            packet[0] = index as u8;
            let start = stream.len();
            stream.extend_from_slice(&packet);
            encode_frame(
                &mut stream,
                start,
                CompressionThreshold(threshold),
                &mut deflate,
            )
            .unwrap();
            packets.push(Bytes::from(packet));
        }
        (stream.to_vec(), packets)
    }

    fn cut<'a>(stream: &'a [u8], offsets: &[usize]) -> Vec<&'a [u8]> {
        let mut parts = Vec::new();
        let mut from = 0;
        for &offset in offsets {
            parts.push(&stream[from..offset]);
            from = offset;
        }
        parts.push(&stream[from..]);
        parts
    }

    fn decode_in_parts(parts: &[&[u8]]) -> Result<Vec<Bytes>, String> {
        let mut buf = BytesMut::new();
        let mut threshold = CompressionThreshold(-1);
        let mut inflater = None;
        let mut packets = Vec::new();
        for (index, part) in parts.iter().enumerate() {
            buf.extend_from_slice(part);
            loop {
                let before = buf.clone();
                match split_frame(&mut buf) {
                    Ok(frame) => {
                        let packet = decompress(frame, threshold, &mut inflater)
                            .map_err(|error| format!("part {index}: {error:?}"))?;
                        packets.push(packet);
                        if packets.len() == SWITCH_AFTER_FRAME + 1 {
                            threshold = CompressionThreshold(256);
                        }
                    }
                    Err(error) if error.is_incomplete() => {
                        if buf != before {
                            return Err(format!("part {index}: {error:?} changed the buffer"));
                        }
                        break;
                    }
                    Err(error) => return Err(format!("part {index}: {error:?}")),
                }
            }
        }
        if !buf.is_empty() {
            return Err(format!("{} bytes left over", buf.len()));
        }
        Ok(packets)
    }

    #[test]
    fn a_stream_cut_anywhere_decodes_to_the_same_frames() {
        let (stream, packets) = five_frame_stream();

        assert_eq!(decode_in_parts(&[&stream]), Ok(packets.clone()));

        for offset in 1..stream.len() {
            assert_eq!(
                decode_in_parts(&cut(&stream, &[offset])),
                Ok(packets.clone()),
                "cut at {offset} of {}",
                stream.len()
            );
        }

        for seed in 0..200u64 {
            let mut rng = StdRng::seed_from_u64(seed);
            let count = rng.random_range(2..=12);
            let mut offsets: Vec<usize> = (0..count)
                .map(|_| rng.random_range(1..stream.len()))
                .collect();
            offsets.sort_unstable();
            offsets.dedup();
            assert_eq!(
                decode_in_parts(&cut(&stream, &offsets)),
                Ok(packets.clone()),
                "seed {seed}, cuts {offsets:?} of {}",
                stream.len()
            );
        }
    }
}
