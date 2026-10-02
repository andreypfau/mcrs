use bytes::{Buf, Bytes, BytesMut};
use thiserror::Error;

pub const MAX_FRAME_BODY: usize = 2_097_151;

pub const MAX_UNCOMPRESSED_PACKET: usize = 8_388_608;

const MAX_LENGTH_BYTES: usize = 3;

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

#[cfg(test)]
mod tests {
    use super::*;

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
}
