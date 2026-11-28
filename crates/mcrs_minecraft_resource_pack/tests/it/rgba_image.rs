use mcrs_minecraft_resource_pack::RgbaImage;
use png::{BitDepth, ColorType, Encoder, EncodingError};

struct Case {
    color: ColorType,
    depth: BitDepth,
    palette: &'static [u8],
    trns: &'static [u8],
    data: &'static [u8],
    rgba: [u8; 8],
}

fn encode(case: &Case) -> Result<Vec<u8>, EncodingError> {
    let mut bytes = Vec::new();
    let mut encoder = Encoder::new(&mut bytes, 2, 1);
    encoder.set_color(case.color);
    encoder.set_depth(case.depth);
    if !case.palette.is_empty() {
        encoder.set_palette(case.palette);
    }
    if !case.trns.is_empty() {
        encoder.set_trns(case.trns);
    }
    let mut writer = encoder.write_header()?;
    writer.write_image_data(case.data)?;
    writer.finish()?;
    Ok(bytes)
}

#[test]
fn every_png_layout_decodes_to_eight_bit_rgba() {
    let cases = [
        Case {
            color: ColorType::Indexed,
            depth: BitDepth::Eight,
            palette: &[10, 20, 30, 40, 50, 60],
            trns: &[128],
            data: &[0, 1],
            rgba: [10, 20, 30, 128, 40, 50, 60, 255],
        },
        Case {
            color: ColorType::Indexed,
            depth: BitDepth::Two,
            palette: &[1, 2, 3, 4, 5, 6],
            trns: &[],
            data: &[0b0100_0000],
            rgba: [4, 5, 6, 255, 1, 2, 3, 255],
        },
        Case {
            color: ColorType::Grayscale,
            depth: BitDepth::One,
            palette: &[],
            trns: &[],
            data: &[0b1000_0000],
            rgba: [255, 255, 255, 255, 0, 0, 0, 255],
        },
        Case {
            color: ColorType::Grayscale,
            depth: BitDepth::Eight,
            palette: &[],
            trns: &[0, 9],
            data: &[9, 7],
            rgba: [9, 9, 9, 0, 7, 7, 7, 255],
        },
        Case {
            color: ColorType::GrayscaleAlpha,
            depth: BitDepth::Eight,
            palette: &[],
            trns: &[],
            data: &[7, 9, 200, 100],
            rgba: [7, 7, 7, 9, 200, 200, 200, 100],
        },
        Case {
            color: ColorType::Rgb,
            depth: BitDepth::Sixteen,
            palette: &[],
            trns: &[],
            data: &[0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC, 1, 0, 2, 0, 3, 0],
            rgba: [0x12, 0x56, 0x9A, 255, 1, 2, 3, 255],
        },
    ];
    for case in &cases {
        let image = RgbaImage::decode_png(&encode(case).unwrap()).unwrap();
        assert_eq!(
            (image.width, image.height, image.rgba.as_slice()),
            (2, 1, case.rgba.as_slice()),
            "{:?} at {:?}",
            case.color,
            case.depth
        );
    }
}
