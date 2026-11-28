use std::io::Cursor;

use png::{BitDepth, ColorType, Decoder, Transformations};

use crate::png_error::PngError;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RgbaImage {
    pub rgba: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

impl RgbaImage {
    pub fn decode_png(bytes: &[u8]) -> Result<Self, PngError> {
        let mut decoder = Decoder::new(Cursor::new(bytes));
        decoder.set_transformations(Transformations::ALPHA | Transformations::STRIP_16);
        let mut reader = decoder.read_info()?;
        let size = reader.output_buffer_size().ok_or(PngError::TooLarge)?;
        let mut pixels = vec![0; size];
        let frame = reader.next_frame(&mut pixels)?;
        pixels.truncate(frame.buffer_size());
        let rgba = match (frame.color_type, frame.bit_depth) {
            (ColorType::Rgba, BitDepth::Eight) => pixels,
            (ColorType::GrayscaleAlpha, BitDepth::Eight) => pixels
                .as_chunks::<2>()
                .0
                .iter()
                .flat_map(|&[gray, alpha]| [gray, gray, gray, alpha])
                .collect(),
            _ => return Err(PngError::Layout),
        };
        Ok(Self {
            rgba,
            width: frame.width,
            height: frame.height,
        })
    }
}
