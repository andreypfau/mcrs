use mcrs_minecraft_client_jar::Directory;
use serde::Deserialize;

use crate::font_definition::{CodePoint, Filter};
use crate::font_glyph::{Glyph, GlyphImage, ImageSource};

/// Rows of a Unifont glyph, and the columns the glyph cache holds for one.
pub(crate) const CELL: usize = 16;
pub(crate) const CELL_BYTES: usize = CELL * CELL * 4;

#[derive(Deserialize)]
pub(crate) struct UnihexSpec {
    #[serde(default)]
    pub(crate) filter: Filter,
    pub(crate) hex_file: String,
    #[serde(default)]
    size_overrides: Vec<SizeOverride>,
}

impl UnihexSpec {
    /// FONT-PROV-008: each override range runs upwards.
    pub(crate) fn check(&self) -> Result<(), String> {
        match self
            .size_overrides
            .iter()
            .find(|size| size.from.0 >= size.to.0)
        {
            Some(size) => Err(format!(
                "{}: an override from {:?} is not below {:?}",
                self.hex_file, size.from.0, size.to.0
            )),
            None => Ok(()),
        }
    }
}

#[derive(Deserialize)]
struct SizeOverride {
    from: CodePoint,
    to: CodePoint,
    left: i32,
    right: i32,
}

#[derive(Clone, Copy, Debug)]
struct Entry {
    code_point: u32,
    offset: usize,
    columns: u32,
    left: i32,
    right: i32,
}

/// The glyphs of the `.hex` entries of a Unifont archive, as bits. Only the glyphs a text asks for
/// become pixels, and only when they are drawn.
#[derive(Debug)]
pub(crate) struct Unihex {
    entries: Vec<Entry>,
    rows: Vec<u8>,
}

impl Unihex {
    pub(crate) fn new(spec: &UnihexSpec, archive: &[u8]) -> Result<Self, String> {
        let directory = Directory::of(archive)?;
        let mut entries = Vec::new();
        let mut rows = Vec::new();
        for file in directory.inflated(archive, |name| name.ends_with(".hex")) {
            let (name, bytes) = file?;
            let text = std::str::from_utf8(&bytes).map_err(|error| format!("{name}: {error}"))?;
            read_lines(text, &mut entries, &mut rows)
                .map_err(|error| format!("{name}: {error}"))?;
        }
        // The later line of a code point wins; a stable sort keeps lines in file order.
        entries.sort_by_key(|entry| entry.code_point);
        let mut unique: Vec<Entry> = Vec::with_capacity(entries.len());
        for entry in entries {
            match unique.last_mut() {
                Some(last) if last.code_point == entry.code_point => *last = entry,
                _ => unique.push(entry),
            }
        }
        let mut hex = Self {
            entries: unique,
            rows,
        };
        hex.override_sizes(&spec.size_overrides);
        Ok(hex)
    }

    fn override_sizes(&mut self, overrides: &[SizeOverride]) {
        let mut taken = vec![false; self.entries.len()];
        for size in overrides {
            let (from, to) = (u32::from(size.from.0), u32::from(size.to.0));
            let first = self
                .entries
                .partition_point(|entry| entry.code_point < from);
            for (entry, taken) in self.entries.iter_mut().zip(taken.iter_mut()).skip(first) {
                if entry.code_point > to {
                    break;
                }
                if !*taken {
                    *taken = true;
                    (entry.left, entry.right) = (size.left, size.right);
                }
            }
        }
    }

    fn entry(&self, ch: char) -> Option<&Entry> {
        self.entries
            .binary_search_by_key(&u32::from(ch), |entry| entry.code_point)
            .ok()
            .and_then(|at| self.entries.get(at))
    }

    pub(crate) fn glyph(&self, source: usize, ch: char) -> Option<Glyph> {
        let entry = self.entry(ch)?;
        let width = entry.right - entry.left + 1;
        Some(Glyph {
            advance: (width / 2 + 1) as f32,
            offset: 0.5,
            image: u32::try_from(width)
                .ok()
                .filter(|&columns| columns > 0)
                .map(|columns| GlyphImage {
                    source: ImageSource::Unifont { source, columns },
                    left: 0.0,
                    right: columns as f32 / 2.0,
                    top: 0.0,
                    bottom: 8.0,
                }),
        })
    }

    /// Draws the glyph into `out`, [`CELL`] × [`CELL`] white pixels with alpha where the glyph is
    /// set, from its first column. Returns whether the glyph exists.
    // chisle: a glyph wider than 16 columns is cut at 16; only a blank one has 17 in the vanilla
    // data. A wider cell lifts it.
    pub(crate) fn rasterise(&self, ch: char, out: &mut [u8; CELL_BYTES]) -> bool {
        let Some(entry) = self.entry(ch) else {
            return false;
        };
        out.fill(0);
        for row in 0..CELL {
            let bits = self.row_bits(entry, row);
            for k in 0..CELL as i32 {
                let column = entry.left + k;
                if column > entry.right || !(0..entry.columns as i32).contains(&column) {
                    continue;
                }
                let at = (row * CELL + k as usize) * 4;
                if (bits >> (entry.columns - 1 - column as u32)) & 1 == 1
                    && let Some(pixel) = out.get_mut(at..at + 4)
                {
                    pixel.fill(255);
                }
            }
        }
        true
    }

    fn row_bits(&self, entry: &Entry, row: usize) -> u32 {
        let bytes = (entry.columns / 8) as usize;
        let start = entry.offset + row * bytes;
        self.rows
            .get(start..start + bytes)
            .unwrap_or_default()
            .iter()
            .fold(0, |bits, byte| (bits << 8) | u32::from(*byte))
    }

    /// The code point and the width in pixels of every glyph, in code point order.
    #[cfg(test)]
    pub(crate) fn widths(&self) -> impl Iterator<Item = (u32, i32)> + '_ {
        self.entries
            .iter()
            .map(|entry| (entry.code_point, entry.right - entry.left + 1))
    }

    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.entries.len()
    }
}

fn read_lines(text: &str, entries: &mut Vec<Entry>, rows: &mut Vec<u8>) -> Result<(), String> {
    for (number, line) in text.lines().enumerate() {
        let number = number + 1;
        let (code_point, data) = line
            .split_once(':')
            .ok_or_else(|| format!("line {number} has no colon"))?;
        if !(4..=6).contains(&code_point.len()) || !is_upper_hex(code_point) {
            return Err(format!("line {number} has a bad code point"));
        }
        let code_point = u32::from_str_radix(code_point, 16)
            .ok()
            .filter(|&value| value <= 0x10_FFFF)
            .ok_or_else(|| format!("line {number} has a bad code point"))?;
        let columns = match data.len() {
            32 => 8,
            64 => 16,
            96 => 24,
            128 => 32,
            other => return Err(format!("line {number} has {other} digits of data")),
        };
        if !is_upper_hex(data) {
            return Err(format!("line {number} has a bad digit in its data"));
        }
        let offset = rows.len();
        rows.extend(
            data.as_bytes()
                .as_chunks::<2>()
                .0
                .iter()
                .map(|[high, low]| (nibble(*high) << 4) | nibble(*low)),
        );
        let (left, right) = bounds(rows.get(offset..).unwrap_or_default(), columns);
        entries.push(Entry {
            code_point,
            offset,
            columns,
            left,
            right,
        });
    }
    Ok(())
}

fn is_upper_hex(text: &str) -> bool {
    text.bytes()
        .all(|byte| byte.is_ascii_digit() || (b'A'..=b'F').contains(&byte))
}

fn nibble(digit: u8) -> u8 {
    if digit.is_ascii_digit() {
        digit - b'0'
    } else {
        digit - b'A' + 10
    }
}

/// The leftmost and the rightmost column with a set pixel in any of the 16 rows; a blank glyph
/// reaches one column past its data.
fn bounds(rows: &[u8], columns: u32) -> (i32, i32) {
    let bytes = (columns / 8) as usize;
    let any = rows
        .chunks_exact(bytes)
        .take(CELL)
        .flat_map(|row| row.iter().enumerate())
        .fold(0u32, |any, (at, byte)| {
            any | (u32::from(*byte) << (8 * (bytes - 1 - at)))
        });
    if any == 0 {
        return (0, columns as i32);
    }
    let left = columns - (32 - any.leading_zeros());
    let right = columns - 1 - any.trailing_zeros();
    (left as i32, right as i32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_glyph_is_drawn_from_its_first_inked_column_in_row_order() {
        let line = format!("0041:38{}", "00".repeat(15));
        let (mut entries, mut rows) = (Vec::new(), Vec::new());
        read_lines(&line, &mut entries, &mut rows).unwrap();
        let hex = Unihex { entries, rows };
        let mut cell = [0; CELL_BYTES];
        assert!(hex.rasterise('A', &mut cell));
        let inked: Vec<(usize, usize)> = (0..CELL * CELL)
            .filter(|at| cell[at * 4 + 3] == 255)
            .map(|at| (at % CELL, at / CELL))
            .collect();
        assert_eq!(inked, [(0, 0), (1, 0), (2, 0)]);
        assert_eq!(hex.glyph(0, 'A').unwrap().advance, 2.0);
    }
}
