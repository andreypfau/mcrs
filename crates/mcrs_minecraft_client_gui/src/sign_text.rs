//! The text of signs and hanging signs (`docs/spec/client-sign-text.md`): which lines show, where
//! they sit in the block, their colours and light, and the glyph quads that draw them. The module
//! has no renderer: it gives quads in block space that a pass turns into a mesh.

use bevy::math::{DMat4, DVec3, IVec2, Vec3};
use mcrs_minecraft_block_entity::{SignText, Text as StoredText};
use mcrs_minecraft_client_world::sign::{SignFamily, SignMount};
use mcrs_minecraft_protocol_game::text::{IntoText, Text};
use mcrs_voxel::BlockPos;

use crate::font::{Font, Span};
use crate::language::ClientLanguage;
use crate::text::{Wrap, text_spans, wrap};

/// SIGN-LIGHT-001 item 1: block light 15, sky light 15.
pub const FULL_BRIGHT_LIGHT: u32 = 15_728_880;
/// SIGN-COL-002 item 3.
pub const BLACK_GLOW_OUTLINE: u32 = 0xFFF0_EBCC;
/// SIGN-COL-003: the squared distance, in blocks, under which a glowing sign has its outline.
pub const OUTLINE_DISTANCE_SQUARED: f64 = 256.0;
/// SIGN-FONT-002 item 2: a hollow rectangle 5 texels wide and 8 high, and its advance. The advance
/// is the spec's open item 3, taken as the width plus one.
pub const MISSING_ADVANCE: i32 = 6;
const MISSING_SIZE: IVec2 = IVec2::new(5, 8);

/// How far the line stands before its outline copies, in text units. A pass that cannot offset
/// depth draws the outline behind the line with this (SIGN-LIGHT-001 item 3).
const LINE_LIFT: f64 = 0.02;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Front,
    Back,
}

/// SIGN-LAY-001: the line height in text units.
pub fn line_height(family: SignFamily) -> i32 {
    match family {
        SignFamily::Standing => 10,
        SignFamily::Hanging => 9,
    }
}

/// SIGN-LAY-001: the maximum line width in text units.
pub fn max_line_width(family: SignFamily) -> i32 {
    match family {
        SignFamily::Standing => 90,
        SignFamily::Hanging => 60,
    }
}

/// SIGN-PLACE-001: the matrix from the text plane of one side to block space. `angle` is the
/// angle `A` in degrees.
pub fn text_matrix(family: SignFamily, mount: SignMount, angle: f32, side: Side) -> DMat4 {
    let turn = DMat4::from_rotation_y(-f64::from(angle).to_radians());
    let back = match side {
        Side::Front => DMat4::IDENTITY,
        Side::Back => DMat4::from_rotation_y(std::f64::consts::PI),
    };
    let at = |x: f64, y: f64, z: f64| DMat4::from_translation(DVec3::new(x, y, z));
    match family {
        SignFamily::Standing => {
            let k = 0.010_416_667_f64;
            let wall = match mount {
                SignMount::Wall => at(0.0, -0.3125, -0.4375),
                SignMount::Ground | SignMount::Ceiling => DMat4::IDENTITY,
            };
            at(0.5, 0.5, 0.5)
                * turn
                * wall
                * back
                * at(0.0, 0.333_333_34, 0.046_666_667)
                * DMat4::from_scale(DVec3::new(k, -k, k))
        }
        SignFamily::Hanging => {
            let k = 0.0140625_f64;
            at(0.5, 0.9375, 0.5)
                * turn
                * at(0.0, -0.3125, 0.0)
                * back
                * at(0.0, -0.32, 0.073)
                * DMat4::from_scale(DVec3::new(k, -k, k))
        }
    }
}

/// SIGN-LAY-002: where line `index` starts on the text plane, for a line `width` units wide.
pub fn line_origin(index: i32, height: i32, width: i32) -> IVec2 {
    IVec2::new(-(width / 2), index * height - (4 * height) / 2)
}

/// SIGN-COL-001: the bright and the dark ARGB colour of a dye. An unknown name is `black`.
pub fn dye_colors(dye: &str) -> (u32, u32) {
    let bright: u32 = match dye {
        "white" => 0xFFFF_FFFF,
        "orange" => 0xFFFF_681F,
        "magenta" => 0xFFFF_00FF,
        "light_blue" => 0xFF9A_C0CD,
        "yellow" => 0xFFFF_FF00,
        "lime" => 0xFFBF_FF00,
        "pink" => 0xFFFF_69B4,
        "gray" => 0xFF80_8080,
        "light_gray" => 0xFFD3_D3D3,
        "cyan" => 0xFF00_FFFF,
        "purple" => 0xFFA0_20F0,
        "blue" => 0xFF00_00FF,
        "brown" => 0xFF8B_4513,
        "green" => 0xFF00_FF00,
        "red" => 0xFFFF_0000,
        _ => 0xFF00_0000,
    };
    let dark = |shift: u32| (((bright >> shift) & 0xFF) * 4 / 10) << shift;
    (bright, 0xFF00_0000 | dark(16) | dark(8) | dark(0))
}

/// SIGN-COL-003: whether the outline of a glowing sign shows. `camera` is the position of the
/// camera entity, when there is one.
pub fn outline_in_range(camera: Option<DVec3>, block: BlockPos, spyglass: bool) -> bool {
    spyglass
        || camera.is_some_and(|camera| {
            let centre = DVec3::new(
                f64::from(block.x) + 0.5,
                f64::from(block.y) + 0.5,
                f64::from(block.z) + 0.5,
            );
            camera.distance_squared(centre) < OUTLINE_DISTANCE_SQUARED
        })
}

/// The colours and the light of the lines of one side.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Paint {
    /// The colour of a code point whose style sets none.
    pub color: u32,
    pub outline: Option<u32>,
    pub light: u32,
}

/// SIGN-COL-002 and SIGN-LIGHT-001. `block_light` is the packed light of the block position.
pub fn paint(dye: &str, glowing: bool, outline_in_range: bool, block_light: u32) -> Paint {
    let (bright, dark) = dye_colors(dye);
    if !glowing {
        return Paint {
            color: dark,
            outline: None,
            light: block_light,
        };
    }
    let outline = if dye_is_black(dye) {
        Some(BLACK_GLOW_OUTLINE)
    } else {
        outline_in_range.then_some(dark)
    };
    Paint {
        color: bright,
        outline,
        light: FULL_BRIGHT_LIGHT,
    }
}

fn dye_is_black(dye: &str) -> bool {
    dye_colors(dye).0 == 0xFF00_0000
}

/// The packed light of `block` and `sky` levels.
pub fn packed_light(block: u8, sky: u8) -> u32 {
    u32::from(block) * 16 + u32::from(sky) * 1_048_576
}

/// SIGN-LINE-001 item 1 and 2: the four messages of a side.
pub fn selected(side: &SignText, filtering: bool) -> &[StoredText] {
    match (&side.filtered_messages, filtering) {
        (Some(filtered), true) => filtered,
        _ => &side.messages,
    }
}

fn component(stored: &StoredText) -> Text {
    match stored {
        StoredText::Plain(text) => Text::text(text.clone()),
        StoredText::Rich(tag) => {
            mcrs_minecraft_nbt::from_tag(tag.clone()).unwrap_or_else(|_| "".into_text())
        }
    }
}

/// One line of a sign: its spans, its width and where it starts on the text plane.
#[derive(Clone, Debug, PartialEq)]
pub struct Line {
    pub spans: Vec<Span>,
    pub width: i32,
    pub origin: IVec2,
}

/// SIGN-LINE-001 and SIGN-LAY-002: the four lines of a side. `color` is the default colour of
/// the side (`Paint::color`). A message wraps at the maximum width and its first row is the line;
/// a message with no row is an empty line.
pub fn lines(
    side: &SignText,
    filtering: bool,
    family: SignFamily,
    color: u32,
    font: &Font,
    language: &ClientLanguage,
) -> Vec<Line> {
    let height = line_height(family);
    let mut messages: Vec<Option<&StoredText>> =
        selected(side, filtering).iter().map(Some).collect();
    messages.resize(4, None);
    messages
        .into_iter()
        .take(4)
        .enumerate()
        .map(|(index, message)| {
            let spans = message
                .map(|message| text_spans(&component(message), language, color))
                .unwrap_or_default();
            let mut rows = Vec::new();
            wrap(
                &spans,
                max_line_width(family),
                Wrap::Tooltip,
                font,
                &mut rows,
            );
            let spans = rows.into_iter().next().unwrap_or_default();
            let width = font.width(&spans);
            Line {
                origin: line_origin(index as i32, height, width),
                spans,
                width,
            }
        })
        .collect()
}

/// A glyph drawn on a sign: the corners in block space, in the order top left, top right, bottom
/// right, bottom left of the text plane, and what to sample.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SignQuad {
    pub corners: [Vec3; 4],
    /// The top left texel of the glyph cell of the font page, or `None` for a solid fill.
    pub cell: Option<IVec2>,
    pub color: u32,
    pub light: u32,
}

const OUTLINE_OFFSETS: [IVec2; 8] = [
    IVec2::new(-1, -1),
    IVec2::new(0, -1),
    IVec2::new(1, -1),
    IVec2::new(-1, 0),
    IVec2::new(1, 0),
    IVec2::new(-1, 1),
    IVec2::new(0, 1),
    IVec2::new(1, 1),
];

/// SIGN-COL-002 items 7 and 8, SIGN-FONT-002: the quads of the lines of one side. The outline
/// copies come first, so the line lies over them.
pub fn quads(font: &Font, lines: &[Line], matrix: &DMat4, paint: &Paint, out: &mut Vec<SignQuad>) {
    let size = font.cell_size;
    let mut rect = |at: IVec2, extent: IVec2, lift: f64, cell: Option<IVec2>, color: u32| {
        let corner = |dx: i32, dy: i32| {
            let point = DVec3::new(f64::from(at.x + dx), f64::from(at.y + dy), lift);
            matrix.transform_point3(point).as_vec3()
        };
        out.push(SignQuad {
            corners: [
                corner(0, 0),
                corner(extent.x, 0),
                corner(extent.x, extent.y),
                corner(0, extent.y),
            ],
            cell,
            color,
            light: paint.light,
        });
    };
    let mut draw = |line: &Line, shift: IVec2, lift: f64, forced: Option<u32>| {
        let mut x = line.origin.x + shift.x;
        let y = line.origin.y + shift.y;
        for span in &line.spans {
            let color = forced.unwrap_or(span.color);
            for ch in span.text.chars() {
                let strikes = if span.bold { 2 } else { 1 };
                if font.is_missing(ch) {
                    for strike in 0..strikes {
                        hollow(IVec2::new(x + strike, y), lift, color, &mut rect);
                    }
                } else if let Some(glyph) = font.glyph(ch) {
                    for strike in 0..strikes {
                        rect(
                            IVec2::new(x + strike, y),
                            size,
                            lift,
                            Some(glyph.cell),
                            color,
                        );
                    }
                }
                x += font.advance(ch, span.bold);
            }
        }
    };
    for line in lines {
        if let Some(outline) = paint.outline {
            for shift in OUTLINE_OFFSETS {
                draw(line, shift, 0.0, Some(outline));
            }
        }
        draw(line, IVec2::ZERO, LINE_LIFT, None);
    }
}

/// The missing glyph: four bars one unit thick around a 5 by 8 rectangle.
fn hollow(
    at: IVec2,
    lift: f64,
    color: u32,
    rect: &mut impl FnMut(IVec2, IVec2, f64, Option<IVec2>, u32),
) {
    let (w, h) = (MISSING_SIZE.x, MISSING_SIZE.y);
    rect(at, IVec2::new(w, 1), lift, None, color);
    rect(
        at + IVec2::new(0, h - 1),
        IVec2::new(w, 1),
        lift,
        None,
        color,
    );
    rect(
        at + IVec2::new(0, 1),
        IVec2::new(1, h - 2),
        lift,
        None,
        color,
    );
    rect(
        at + IVec2::new(w - 1, 1),
        IVec2::new(1, h - 2),
        lift,
        None,
        color,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::font::corpus_font;
    use mcrs_minecraft_client_world::sign::yaw_of;
    use serde::Deserialize;

    const ARTEFACT: &str =
        include_str!("../../mcrs_minecraft_model/tests/fixtures/vanilla/sign_text.json");

    #[derive(Deserialize)]
    struct Artefact {
        cases: Vec<Case>,
    }

    #[derive(Deserialize)]
    #[serde(tag = "kind", rename_all = "snake_case")]
    enum Case {
        Constants {
            sign_line_height: i32,
            sign_max_line_width: i32,
            hanging_line_height: i32,
            hanging_max_line_width: i32,
            full_bright_light: i64,
            black_glow_outline_color: i32,
            outline_distance_squared: f64,
        },
        DyeColors {
            dye: String,
            bright: i32,
            dark: i32,
        },
        Transform {
            family: String,
            attachment: String,
            state: String,
            side: String,
            matrix: [[f64; 4]; 4],
        },
        Layout {
            family: String,
            lines: Vec<LayoutLine>,
        },
        Paint {
            family: String,
            dye: String,
            glowing: bool,
            outline_in_range: bool,
            color: i32,
            outline_color: i32,
            light: u32,
            input_light: u32,
        },
        Filtering {
            filtering_enabled: bool,
            lines: Vec<String>,
        },
        Styles {},
        StylesInherited {},
    }

    #[derive(Deserialize)]
    struct LayoutLine {
        advance_total: i32,
        x: f64,
        y: f64,
    }

    fn cases() -> Vec<Case> {
        serde_json::from_str::<Artefact>(ARTEFACT).unwrap().cases
    }

    fn family_of(name: &str) -> SignFamily {
        match name {
            "standing" => SignFamily::Standing,
            "hanging" => SignFamily::Hanging,
            other => panic!("{other}"),
        }
    }

    #[test]
    fn sign_test_001_constants() {
        let mut seen = false;
        for case in cases() {
            if let Case::Constants {
                sign_line_height,
                sign_max_line_width,
                hanging_line_height,
                hanging_max_line_width,
                full_bright_light,
                black_glow_outline_color,
                outline_distance_squared,
            } = case
            {
                seen = true;
                assert_eq!(line_height(SignFamily::Standing), sign_line_height);
                assert_eq!(max_line_width(SignFamily::Standing), sign_max_line_width);
                assert_eq!(line_height(SignFamily::Hanging), hanging_line_height);
                assert_eq!(max_line_width(SignFamily::Hanging), hanging_max_line_width);
                assert_eq!(i64::from(FULL_BRIGHT_LIGHT), full_bright_light);
                assert_eq!(BLACK_GLOW_OUTLINE, black_glow_outline_color as u32);
                assert_eq!(OUTLINE_DISTANCE_SQUARED, outline_distance_squared);
            }
        }
        assert!(seen);
    }

    #[test]
    fn sign_test_001_dye_colours() {
        let mut rows = 0;
        for case in cases() {
            if let Case::DyeColors { dye, bright, dark } = case {
                assert_eq!(dye_colors(&dye), (bright as u32, dark as u32), "{dye}");
                rows += 1;
            }
        }
        assert_eq!(rows, 16);
    }

    #[test]
    fn sign_test_001_text_matrices() {
        let mut rows = 0;
        for case in cases() {
            let Case::Transform {
                family,
                attachment,
                state,
                side,
                matrix,
            } = case
            else {
                continue;
            };
            let mount = match attachment.as_str() {
                "ground" => SignMount::Ground,
                "wall" => SignMount::Wall,
                "ceiling" => SignMount::Ceiling,
                other => panic!("{other}"),
            };
            let angle = match mount {
                SignMount::Wall => yaw_of(&state).unwrap(),
                _ => state.parse::<f32>().unwrap() * 22.5,
            };
            let side_of = if side == "front" {
                Side::Front
            } else {
                Side::Back
            };
            let got = text_matrix(family_of(&family), mount, angle, side_of);
            for (row, expected) in matrix.iter().enumerate() {
                for (column, expected) in expected.iter().enumerate() {
                    let value = got.col(column)[row];
                    assert!(
                        (value - expected).abs() < 1e-6,
                        "{family} {attachment} {state} {side} [{row}][{column}]: {value} vs {expected}"
                    );
                }
            }
            rows += 1;
        }
        assert_eq!(rows, 80);
    }

    #[test]
    fn sign_test_001_line_positions() {
        let mut rows = 0;
        for case in cases() {
            let Case::Layout { family, lines } = case else {
                continue;
            };
            let height = line_height(family_of(&family));
            for (index, line) in lines.iter().enumerate() {
                let at = line_origin(index as i32, height, line.advance_total);
                assert_eq!(
                    (f64::from(at.x), f64::from(at.y)),
                    (line.x, line.y),
                    "{family} line {index} width {}",
                    line.advance_total
                );
            }
            rows += 1;
        }
        assert_eq!(rows, 4);
    }

    #[test]
    fn sign_test_001_colours_outline_and_light() {
        let mut rows = 0;
        for case in cases() {
            let Case::Paint {
                family,
                dye,
                glowing,
                outline_in_range,
                color,
                outline_color,
                light,
                input_light,
            } = case
            else {
                continue;
            };
            assert!(matches!(family.as_str(), "standing" | "hanging"));
            let got = paint(&dye, glowing, outline_in_range, input_light);
            assert_eq!(
                got.color, color as u32,
                "{dye} {glowing} {outline_in_range}"
            );
            assert_eq!(
                got.outline.unwrap_or(0),
                outline_color as u32,
                "{dye} {glowing} {outline_in_range}"
            );
            assert_eq!(got.light, light, "{dye} {glowing} {outline_in_range}");
            rows += 1;
        }
        assert_eq!(rows, 128);
    }

    fn plain(text: &str) -> StoredText {
        StoredText::Plain(text.to_owned())
    }

    #[test]
    fn sign_test_001_filtering_selects_one_of_two_lists() {
        let mut rows = 0;
        for case in cases() {
            let Case::Filtering {
                filtering_enabled,
                lines,
            } = case
            else {
                continue;
            };
            let side = SignText {
                messages: ["raw1", "raw2", "raw3", "raw4"].map(plain).to_vec(),
                filtered_messages: Some(["cln1", "cln2", "cln3", "cln4"].map(plain).to_vec()),
                ..SignText::default()
            };
            let shown: Vec<_> = selected(&side, filtering_enabled)
                .iter()
                .map(|text| match text {
                    StoredText::Plain(text) => text.clone(),
                    StoredText::Rich(_) => panic!(),
                })
                .collect();
            assert_eq!(shown, lines);
            rows += 1;
        }
        assert_eq!(rows, 2);
    }

    #[test]
    fn a_side_with_no_filtered_messages_shows_its_messages_either_way() {
        let side = SignText {
            messages: ["a", "b", "c", "d"].map(plain).to_vec(),
            ..SignText::default()
        };
        assert_eq!(selected(&side, true), selected(&side, false));
    }

    #[test]
    fn the_outline_range_is_sixteen_blocks_or_a_spyglass() {
        let block = BlockPos::new(0, 0, 0);
        let at = |x: f64| Some(DVec3::new(x, 0.5, 0.5));
        assert!(outline_in_range(at(15.4), block, false));
        assert!(!outline_in_range(at(16.6), block, false));
        assert!(!outline_in_range(None, block, false));
        assert!(outline_in_range(at(40.0), block, true));
    }

    mod jar {
        use super::*;

        fn side(messages: [&str; 4]) -> SignText {
            SignText {
                messages: messages.map(plain).to_vec(),
                ..SignText::default()
            }
        }

        #[test]
        fn a_message_shows_only_the_first_row_of_its_wrap() {
            let font = corpus_font();
            let wide = "wide wide wide wide wide wide wide wide";
            let got = lines(
                &side(["Hello", wide, "", "x"]),
                false,
                SignFamily::Standing,
                0xFF00_0000,
                font,
                ClientLanguage::corpus(),
            );
            assert_eq!(got.len(), 4);
            assert_eq!(got[0].width, font.str_width("Hello"));
            assert!(got[1].width <= 90 && got[1].width > 60, "{}", got[1].width);
            assert_eq!(got[2].spans, []);
            assert_eq!(got[2].origin, IVec2::new(0, 0));
            let hanging = lines(
                &side(["", wide, "", ""]),
                false,
                SignFamily::Hanging,
                0xFF00_0000,
                font,
                ClientLanguage::corpus(),
            );
            assert!(hanging[1].width <= 60);
        }

        #[test]
        fn a_style_colour_replaces_the_default_and_a_key_with_no_translation_shows_as_the_key() {
            let font = corpus_font();
            let mut compound = mcrs_minecraft_nbt::compound::NbtCompound::new();
            compound.put_string("text", "red".to_owned());
            compound.put_string("color", "red".to_owned());
            let mut translated = mcrs_minecraft_nbt::compound::NbtCompound::new();
            translated.put_string("translate", "sign.x.y".to_owned());
            let mut side = side(["", "", "", ""]);
            side.messages[0] =
                StoredText::Rich(mcrs_minecraft_nbt::tag::NbtTag::Compound(compound));
            side.messages[1] =
                StoredText::Rich(mcrs_minecraft_nbt::tag::NbtTag::Compound(translated));
            let lines = lines(
                &side,
                false,
                SignFamily::Standing,
                0xFF11_1111,
                font,
                ClientLanguage::corpus(),
            );
            assert_eq!(lines[0].spans, [Span::new("red", 0xFFFF_5555)]);
            assert_eq!(lines[1].spans, [Span::new("sign.x.y", 0xFF11_1111)]);
        }

        #[test]
        fn a_child_inherits_colour_and_bold_and_a_missing_key_shows_its_fallback() {
            let font = corpus_font();
            let nbt = |entries: &[(&str, &str)]| {
                let mut compound = mcrs_minecraft_nbt::compound::NbtCompound::new();
                for (key, value) in entries {
                    compound.put_string(key, (*value).to_owned());
                }
                compound
            };
            let mut parent = nbt(&[("text", "p"), ("color", "red")]);
            parent.put_byte("bold", 1);
            let child = nbt(&[("text", "c")]);
            parent.put_list(
                "extra",
                vec![mcrs_minecraft_nbt::tag::NbtTag::Compound(child)],
            );
            let fallback = nbt(&[("translate", "sign.x.y"), ("fallback", "fallback text")]);
            let mut side = side(["", "", "", ""]);
            side.messages[0] = StoredText::Rich(mcrs_minecraft_nbt::tag::NbtTag::Compound(parent));
            side.messages[1] =
                StoredText::Rich(mcrs_minecraft_nbt::tag::NbtTag::Compound(fallback));
            let got = lines(
                &side,
                false,
                SignFamily::Standing,
                0xFF11_1111,
                font,
                ClientLanguage::corpus(),
            );
            assert_eq!(got[0].spans, [Span::new("pc", 0xFFFF_5555).bold()]);
            assert_eq!(got[1].spans, [Span::new("fallback text", 0xFF11_1111)]);
        }

        #[test]
        fn a_glowing_line_is_drawn_under_its_outline_copies_and_a_missing_glyph_is_a_frame() {
            let pack = mcrs_minecraft_model::resource_set::ResourceSet::corpus();
            let bytes = pack.read(crate::font::ASCII_TEXTURE).unwrap();
            let (pixels, width, height) =
                mcrs_minecraft_model::atlas::decode_png(bytes, crate::font::ASCII_TEXTURE).unwrap();
            let font = Font::load(pack, &pixels, width, height)
                .unwrap()
                .with_missing_advance(MISSING_ADVANCE);
            let line = Line {
                spans: vec![Span::new("a\u{e000}", 0xFF00_00FF)],
                width: font.str_width("a\u{e000}"),
                origin: IVec2::ZERO,
            };
            let paint = Paint {
                color: 0xFF00_00FF,
                outline: Some(0xFF11_2233),
                light: FULL_BRIGHT_LIGHT,
            };
            let mut out = Vec::new();
            quads(&font, &[line], &DMat4::IDENTITY, &paint, &mut out);
            // eight copies of two drawn things, then the line: one glyph and four bars.
            assert_eq!(out.len(), 8 * 5 + 5);
            assert!(out[..40].iter().all(|quad| quad.color == 0xFF11_2233));
            assert!(out[40..].iter().all(|quad| quad.color == 0xFF00_00FF));
            assert!(out[40].cell.is_some());
            assert!(out[41..].iter().all(|quad| quad.cell.is_none()));
            assert!(out[40].corners[0].z > out[0].corners[0].z);
            assert_eq!(font.advance('\u{e000}', false), MISSING_ADVANCE);
        }
    }
}
