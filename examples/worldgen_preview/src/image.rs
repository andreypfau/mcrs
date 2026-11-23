use png::{BitDepth, ColorType, Encoder, EncodingError};

#[derive(Clone, Copy)]
pub struct Rgba(pub [u8; 4]);

impl Rgba {
    pub const CLEAR: Rgba = Rgba([0, 0, 0, 0]);

    pub fn opaque(red: u8, green: u8, blue: u8) -> Self {
        Self([red, green, blue, 0xff])
    }
}

pub struct Image {
    width: u32,
    height: u32,
    pixels: Vec<Rgba>,
}

impl Image {
    pub fn new(width: u32, height: u32) -> Self {
        let count = usize::try_from(u64::from(width) * u64::from(height)).unwrap_or(0);
        Self {
            width,
            height,
            pixels: vec![Rgba::CLEAR; count],
        }
    }

    pub fn set(&mut self, x: u32, y: u32, color: Rgba) {
        if let Some(pixel) = self
            .index(x, y)
            .and_then(|index| self.pixels.get_mut(index))
        {
            *pixel = color;
        }
    }

    fn index(&self, x: u32, y: u32) -> Option<usize> {
        if x >= self.width {
            return None;
        }
        let index = y.checked_mul(self.width)?.checked_add(x)?;
        usize::try_from(index).ok()
    }

    pub fn png(&self) -> Result<Vec<u8>, EncodingError> {
        let mut png = Vec::new();
        let mut encoder = Encoder::new(&mut png, self.width, self.height);
        encoder.set_color(ColorType::Rgba);
        encoder.set_depth(BitDepth::Eight);
        let mut writer = encoder.write_header()?;
        let data: Vec<u8> = self.pixels.iter().flat_map(|pixel| pixel.0).collect();
        writer.write_image_data(&data)?;
        writer.finish()?;
        Ok(png)
    }
}
