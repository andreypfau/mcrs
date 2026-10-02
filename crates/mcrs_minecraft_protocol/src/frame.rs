use bytes::{Buf, Bytes, BytesMut};
use flate2::{Decompress, FlushDecompress, Status};
use thiserror::Error;

use crate::CompressionThreshold;
use crate::var_int::VarInt;

pub const MAX_FRAME_BODY: usize = 2_097_151;

pub const MAX_UNCOMPRESSED_PACKET: usize = 8_388_608;

const MAX_LENGTH_BYTES: usize = 3;

const MIN_INFLATE_STEP: usize = 4096;

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
}

impl FrameError {
    pub fn is_incomplete(&self) -> bool {
        matches!(self, Self::IncompleteLength | Self::ShortBody { .. })
    }
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
    let declared =
        VarInt::decode_partial(&mut rest).map_err(|_| FrameError::MalformedDataLength)?;
    let declared = usize::try_from(declared).map_err(|_| FrameError::MalformedDataLength)?;
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
        let mut buf = BytesMut::new();
        let mut length = body.len();
        loop {
            let low = (length & 0x7f) as u8;
            length >>= 7;
            if length == 0 {
                buf.extend_from_slice(&[low]);
                break;
            }
            buf.extend_from_slice(&[low | 0x80]);
        }
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
    fn only_an_incomplete_length_and_a_short_body_can_be_fixed_by_more_input() {
        let cases = [
            (FrameError::IncompleteLength, true),
            (FrameError::MalformedLength, false),
            (FrameError::ZeroLength, false),
            (FrameError::ShortBody { missing: 1 }, true),
            (FrameError::MalformedDataLength, false),
            (
                FrameError::BelowThreshold {
                    declared: 1,
                    threshold: 2,
                },
                false,
            ),
            (FrameError::AboveMaximum { declared: 1 }, false),
            (
                FrameError::ShortStream {
                    declared: 2,
                    inflated: 1,
                },
                false,
            ),
            (FrameError::TrailingData, false),
            (FrameError::CorruptStream, false),
        ];
        for (error, incomplete) in cases {
            assert_eq!(error.is_incomplete(), incomplete, "{error:?}");
        }
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
    fn uncompressed_frames_pass_at_any_size() {
        let mut inflater = None;
        for threshold in [1usize, 64, 256] {
            for size in [threshold - 1, threshold, threshold + 1, 100_000] {
                let rest = pattern(size);
                assert_eq!(
                    decompress(
                        uncompressed_frame(&rest),
                        CompressionThreshold(threshold as i32),
                        &mut inflater
                    ),
                    Ok(Bytes::from(rest)),
                    "threshold {threshold}, size {size}"
                );
            }
        }
        assert!(inflater.is_none());
    }

    #[test]
    fn compressed_frames_are_accepted_at_the_threshold_and_refused_below_it() {
        let rows: [(i32, usize, bool); 8] = [
            (1, 1, true),
            (1, 2, true),
            (64, 63, false),
            (64, 64, true),
            (64, 65, true),
            (256, 255, false),
            (256, 256, true),
            (256, 257, true),
        ];
        for (threshold, declared, accepted) in rows {
            let data = pattern(declared);
            let frame = compressed_frame(declared as i32, &deflate(&data));
            let result = decompress(frame, CompressionThreshold(threshold), &mut None);
            if accepted {
                assert_eq!(result, Ok(Bytes::from(data)), "{threshold} {declared}");
            } else {
                assert_eq!(
                    result,
                    Err(FrameError::BelowThreshold {
                        declared,
                        threshold: threshold as usize
                    }),
                    "{threshold} {declared}"
                );
            }
        }
    }

    #[test]
    fn a_declared_size_at_the_maximum_is_accepted_and_one_past_it_refused() {
        let threshold = CompressionThreshold(256);
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
}
