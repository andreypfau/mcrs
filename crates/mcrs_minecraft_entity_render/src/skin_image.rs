use std::fmt;

use mcrs_minecraft_model::atlas::decode_png;
use mcrs_minecraft_model::sprite_error::SpriteError;

pub const SKIN_SIDE: u32 = 64;
const ROW: usize = SKIN_SIDE as usize * 4;

const OPAQUE: [[usize; 4]; 3] = [[0, 0, 32, 16], [0, 16, 64, 16], [16, 48, 32, 16]];

const HAT: [usize; 4] = [32, 0, 32, 16];

const LEGACY_LIMBS: [([usize; 2], [usize; 2]); 2] = [([0, 16], [16, 48]), ([40, 16], [32, 48])];

const LIMB_FACES: [([usize; 4], [usize; 2]); 6] = [
    ([4, 0, 4, 4], [4, 0]),
    ([8, 0, 4, 4], [8, 0]),
    ([0, 4, 4, 12], [8, 4]),
    ([4, 4, 4, 12], [4, 4]),
    ([8, 4, 4, 12], [0, 4]),
    ([12, 4, 4, 12], [12, 4]),
];

#[derive(Clone, PartialEq, Eq)]
pub struct SkinImage(Vec<u8>);

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SkinError {
    Decode(SpriteError),
    Size { width: u32, height: u32 },
    Length { expected: usize, found: usize },
}

impl fmt::Display for SkinError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Decode(error) => write!(f, "the skin is not a PNG image: {error}"),
            Self::Size { width, height } => {
                write!(
                    f,
                    "a skin is 64 by 64 or 64 by 32 texels, not {width} by {height}"
                )
            }
            Self::Length { expected, found } => {
                write!(
                    f,
                    "a skin of this size has {expected} RGBA bytes, not {found}"
                )
            }
        }
    }
}

impl std::error::Error for SkinError {}

fn is_skin_size(width: u32, height: u32) -> bool {
    width == SKIN_SIDE && (height == SKIN_SIDE || height == SKIN_SIDE / 2)
}

fn png_size(bytes: &[u8]) -> Option<(u32, u32)> {
    const SIGNATURE: &[u8] = b"\x89PNG\r\n\x1a\n";
    let header = bytes.strip_prefix(SIGNATURE)?.get(4..16)?;
    let (kind, size) = header.split_at(4);
    let (width, height) = size.split_at(4);
    (kind == b"IHDR").then(|| {
        (
            u32::from_be_bytes(width.try_into().unwrap_or_default()),
            u32::from_be_bytes(height.try_into().unwrap_or_default()),
        )
    })
}

impl SkinImage {
    pub fn from_png(bytes: &[u8]) -> Result<Self, SkinError> {
        if let Some((width, height)) = png_size(bytes)
            && !is_skin_size(width, height)
        {
            return Err(SkinError::Size { width, height });
        }
        let (rgba, width, height) = decode_png(bytes, "skin").map_err(SkinError::Decode)?;
        Self::from_rgba(width, height, rgba)
    }

    pub fn from_rgba(width: u32, height: u32, rgba: Vec<u8>) -> Result<Self, SkinError> {
        if !is_skin_size(width, height) {
            return Err(SkinError::Size { width, height });
        }
        let expected = (width * height * 4) as usize;
        if rgba.len() != expected {
            return Err(SkinError::Length {
                expected,
                found: rgba.len(),
            });
        }
        let mut texels = rgba;
        if height == SKIN_SIDE / 2 {
            texels.resize(ROW * SKIN_SIDE as usize, 0);
            for (from, to) in LEGACY_LIMBS {
                mirror_limb(&mut texels, from, to);
            }
            if alphas(&mut texels, HAT).all(|alpha| *alpha >= 128) {
                for alpha in alphas(&mut texels, HAT) {
                    *alpha = 0;
                }
            }
        }
        for rectangle in OPAQUE {
            for alpha in alphas(&mut texels, rectangle) {
                *alpha = 255;
            }
        }
        Ok(Self(texels))
    }

    #[must_use]
    pub fn rgba(&self) -> &[u8] {
        &self.0
    }
}

impl fmt::Debug for SkinImage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SkinImage")
    }
}

fn alphas(texels: &mut [u8], [x, y, width, height]: [usize; 4]) -> impl Iterator<Item = &mut u8> {
    texels
        .as_chunks_mut::<ROW>()
        .0
        .iter_mut()
        .skip(y)
        .take(height)
        .flat_map(move |row| row.as_chunks_mut::<4>().0.iter_mut().skip(x).take(width))
        .map(|[_, _, _, alpha]| alpha)
}

fn mirror_limb(texels: &mut [u8], from: [usize; 2], to: [usize; 2]) {
    for ([x, y, width, height], [target_x, target_y]) in LIMB_FACES {
        for row in 0..height {
            for column in 0..width {
                let source = (from[1] + y + row) * ROW + (from[0] + x + column) * 4;
                let target =
                    (to[1] + target_y + row) * ROW + (to[0] + target_x + width - 1 - column) * 4;
                if source + 4 <= texels.len() && target + 4 <= texels.len() {
                    texels.copy_within(source..source + 4, target);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texel(image: &SkinImage, x: usize, y: usize) -> [u8; 4] {
        let at = y * ROW + x * 4;
        let mut out = [0; 4];
        out.copy_from_slice(&image.rgba()[at..at + 4]);
        out
    }

    #[test]
    fn a_png_that_states_another_size_is_rejected_from_its_header() {
        let mut bytes = b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR".to_vec();
        bytes.extend_from_slice(&65_536u32.to_be_bytes());
        bytes.extend_from_slice(&65_536u32.to_be_bytes());
        assert_eq!(
            SkinImage::from_png(&bytes),
            Err(SkinError::Size {
                width: 65_536,
                height: 65_536
            })
        );
    }

    #[test]
    fn legacy_skin_mirrors_left_limbs_and_opaques_first_layer() {
        let mut rgba = vec![0u8; ROW * 32];
        let mut put = |x: usize, y: usize, value: [u8; 4]| {
            rgba[y * ROW + x * 4..y * ROW + x * 4 + 4].copy_from_slice(&value);
        };
        put(0, 20, [1, 2, 3, 255]);
        put(44, 20, [4, 5, 6, 255]);
        put(8, 8, [7, 8, 9, 0]);
        put(40, 8, [7, 8, 9, 0]);
        let image = SkinImage::from_rgba(64, 32, rgba).unwrap();
        assert_eq!(texel(&image, 16 + 8 + 3, 52), [1, 2, 3, 255]);
        assert_eq!(texel(&image, 32 + 4 + 3, 52), [4, 5, 6, 255]);
        assert_eq!(texel(&image, 8, 8), [7, 8, 9, 255]);
        assert_eq!(texel(&image, 40, 8), [7, 8, 9, 0]);
        assert_eq!(texel(&image, 20, 36), [0, 0, 0, 0]);
    }

    #[test]
    fn a_legacy_skin_with_an_opaque_hat_has_no_hat() {
        let image = SkinImage::from_rgba(64, 32, vec![255; ROW * 32]).unwrap();
        assert_eq!(texel(&image, 40, 8)[3], 0);
        assert_eq!(texel(&image, 8, 8)[3], 255);
    }

    #[test]
    fn an_image_of_another_size_is_not_a_skin() {
        for (width, height) in [(32, 32), (128, 128), (64, 48)] {
            let rgba = vec![0; (width * height * 4) as usize];
            assert_eq!(
                SkinImage::from_rgba(width, height, rgba),
                Err(SkinError::Size { width, height })
            );
        }
    }
}
