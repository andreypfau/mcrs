use std::collections::{BTreeMap, HashMap};

use bevy::log::warn;
use bevy::math::{Rect, URect, UVec2};

use mcrs_minecraft_model::atlas::decode_png;
use mcrs_minecraft_model::resource_set::ResourceSet;
use mcrs_minecraft_model_file::resource_id::{full_id, split_id};
use serde::de::Error as _;
use serde::{Deserialize, Deserializer};

use crate::font_glyph::{Glyph, GlyphImage, ImageSource};
use crate::font_unihex::{SizeOverride, Unihex};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Toggle {
    #[default]
    Off,
    On,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FontOptions {
    pub uniform: Toggle,
    pub jp: Toggle,
}

#[derive(Clone, Copy, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Filter {
    uniform: Option<bool>,
    jp: Option<bool>,
}

impl Filter {
    pub(crate) fn passes(self, options: FontOptions) -> bool {
        let agrees = |wanted: Option<bool>, option: Toggle| {
            wanted.is_none_or(|wanted| wanted == (option == Toggle::On))
        };
        agrees(self.uniform, options.uniform) && agrees(self.jp, options.jp)
    }

    fn overridden_by(self, over: Self) -> Self {
        Self {
            uniform: over.uniform.or(self.uniform),
            jp: over.jp.or(self.jp),
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) enum Kind {
    Space(BTreeMap<char, f32>),
    Bitmap(BitmapPage),
    Unihex(Unihex),
}

#[derive(Clone, Debug)]
pub(crate) struct Source {
    pub(crate) filter: Filter,
    pub(crate) kind: Kind,
}

#[derive(Deserialize)]
struct Definition {
    providers: Vec<Provider>,
}

impl Definition {
    fn check(&self) -> Result<(), String> {
        self.providers
            .iter()
            .try_for_each(|provider| match provider {
                Provider::Bitmap(spec) => spec.check(),
                Provider::Unihex(spec) => spec.check(),
                Provider::Space(_) | Provider::Reference(_) | Provider::Other => Ok(()),
            })
    }
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Provider {
    Bitmap(BitmapSpec),
    Space(SpaceSpec),
    Unihex(UnihexSpec),
    Reference(ReferenceSpec),
    #[serde(other)]
    Other,
}

#[derive(Deserialize)]
struct SpaceSpec {
    #[serde(default)]
    filter: Filter,
    advances: HashMap<CodePoint, f32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct CodePoint(pub(crate) char);

impl<'de> Deserialize<'de> for CodePoint {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        let mut chars = text.chars();
        match (chars.next(), chars.next()) {
            (Some(ch), None) => Ok(Self(ch)),
            _ => Err(D::Error::custom(format!("{text:?} is not one code point"))),
        }
    }
}

#[derive(Deserialize)]
struct ReferenceSpec {
    #[serde(default)]
    filter: Filter,
    id: String,
}

#[derive(Deserialize)]
struct UnihexSizeOverride {
    from: CodePoint,
    to: CodePoint,
    left: i32,
    right: i32,
}

#[derive(Deserialize)]
struct UnihexSpec {
    #[serde(default)]
    filter: Filter,
    hex_file: String,
    #[serde(default)]
    size_overrides: Vec<UnihexSizeOverride>,
}

impl UnihexSpec {
    fn check(&self) -> Result<(), String> {
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

    fn overrides(&self) -> Vec<SizeOverride> {
        self.size_overrides
            .iter()
            .map(|size| SizeOverride {
                from: size.from.0,
                to: size.to.0,
                left: size.left,
                right: size.right,
            })
            .collect()
    }
}

pub(crate) fn sources(pack: &ResourceSet, id: &str) -> Result<Vec<Source>, String> {
    resolve(pack, id, &mut Vec::new())
}

fn resolve(pack: &ResourceSet, id: &str, loading: &mut Vec<String>) -> Result<Vec<Source>, String> {
    let id = full_id(id);
    if loading.contains(&id) {
        return Err(format!("the font {id} references itself"));
    }
    loading.push(id.clone());
    let (namespace, path) = split_id(&id);
    let file = format!("{namespace}/font/{path}.json");
    let bytes = pack
        .read(&file)
        .map_err(|error| format!("the font {id}: {error}"))?;
    let definition = match serde_json::from_slice::<Definition>(bytes)
        .map_err(|error| error.to_string())
        .and_then(|definition| definition.check().map(|()| definition))
    {
        Ok(definition) => definition,
        Err(error) => {
            warn!("{file} is ignored: {error}");
            loading.pop();
            return Ok(Vec::new());
        }
    };
    let mut sources = Vec::new();
    for provider in definition.providers {
        match provider {
            Provider::Other => {
                warn!("{file}: a provider of a kind this client does not read is skipped")
            }
            Provider::Reference(reference) => {
                for mut source in resolve(pack, &reference.id, loading)? {
                    source.filter = source.filter.overridden_by(reference.filter);
                    sources.push(source);
                }
            }
            Provider::Space(spec) => {
                let advances = spec
                    .advances
                    .iter()
                    .map(|(key, advance)| (key.0, *advance))
                    .collect();
                sources.push(Source {
                    filter: spec.filter,
                    kind: Kind::Space(advances),
                });
            }
            Provider::Bitmap(spec) => {
                let (namespace, path) = split_id(&spec.file);
                let texture = format!("{namespace}/textures/{path}");
                let (pixels, width, height) = decode_png(
                    pack.read(&texture).map_err(|error| error.to_string())?,
                    &texture,
                )
                .map_err(|error| error.to_string())?;
                sources.push(Source {
                    filter: spec.filter,
                    kind: Kind::Bitmap(BitmapPage::new(&spec, pixels, width, height)?),
                });
            }
            Provider::Unihex(spec) => {
                let (namespace, path) = split_id(&spec.hex_file);
                let archive = format!("{namespace}/{path}");
                let bytes = pack.read(&archive).map_err(|error| error.to_string())?;
                let overrides = spec.overrides();
                sources.push(Source {
                    filter: spec.filter,
                    kind: Kind::Unihex(
                        Unihex::new(bytes, &overrides)
                            .map_err(|error| format!("{archive}: {error}"))?,
                    ),
                });
            }
        }
    }
    loading.pop();
    Ok(sources)
}

const DEFAULT_HEIGHT: i32 = 8;
const BASELINE: i32 = 7;

#[derive(Deserialize)]
pub(crate) struct BitmapSpec {
    #[serde(default)]
    pub(crate) filter: Filter,
    pub(crate) file: String,
    #[serde(default = "default_height")]
    height: i32,
    ascent: i32,
    chars: Vec<String>,
}

impl BitmapSpec {
    pub(crate) fn check(&self) -> Result<(), String> {
        if self.ascent > self.height {
            return Err(format!(
                "{}: the ascent {} is above the height {}",
                self.file, self.ascent, self.height
            ));
        }
        let columns = self.chars.first().map_or(0, |row| row.chars().count());
        if columns == 0 || self.chars.iter().any(|row| row.chars().count() != columns) {
            return Err(format!("{}: the rows of chars differ in length", self.file));
        }
        Ok(())
    }
}

fn default_height() -> i32 {
    DEFAULT_HEIGHT
}

#[derive(Clone, Copy, Debug)]
struct Cell {
    texels: URect,
    advance: f32,
}

#[derive(Clone, Debug)]
pub struct BitmapPage {
    pixels: Vec<u8>,
    width: u32,
    height: u32,
    scale: f32,
    top: f32,
    cells: BTreeMap<char, Cell>,
}

impl BitmapPage {
    pub(crate) fn new(
        spec: &BitmapSpec,
        pixels: Vec<u8>,
        width: u32,
        height: u32,
    ) -> Result<Self, String> {
        let columns = spec.chars.first().map_or(0, |row| row.chars().count()) as u32;
        let rows = spec.chars.len() as u32;
        let (cell_width, cell_height) = (width / columns, height / rows);
        if cell_width == 0 || cell_height == 0 || spec.height <= 0 {
            return Err(format!(
                "{}: the texture is smaller than its grid",
                spec.file
            ));
        }
        let scale = spec.height as f32 / cell_height as f32;
        let mut cells = BTreeMap::new();
        for (row, line) in spec.chars.iter().enumerate() {
            for (column, ch) in line.chars().enumerate() {
                if ch == '\0' {
                    continue;
                }
                let min = UVec2::new(column as u32 * cell_width, row as u32 * cell_height);
                let texels = URect::from_corners(min, min + UVec2::new(cell_width, cell_height));
                let ink = ink_width(&pixels, width, texels);
                let advance = (0.5 + ink as f32 * scale).floor() + 1.0;
                cells.insert(ch, Cell { texels, advance });
            }
        }
        Ok(Self {
            pixels,
            width,
            height,
            scale,
            top: (BASELINE - spec.ascent) as f32,
            cells,
        })
    }

    pub fn pixels(&self) -> &[u8] {
        &self.pixels
    }

    pub fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    pub(crate) fn glyph(&self, source: usize, ch: char) -> Option<Glyph> {
        let cell = self.cells.get(&ch)?;
        let texels = cell.texels;
        Some(Glyph {
            advance: cell.advance,
            offset: 1.0,
            image: Some(GlyphImage {
                source: ImageSource::Bitmap {
                    source,
                    texels: Rect::new(
                        texels.min.x as f32,
                        texels.min.y as f32,
                        texels.max.x as f32,
                        texels.max.y as f32,
                    ),
                },
                left: 0.0,
                right: texels.width() as f32 * self.scale,
                top: self.top,
                bottom: self.top + texels.height() as f32 * self.scale,
            }),
        })
    }
}

fn ink_width(pixels: &[u8], width: u32, cell: URect) -> u32 {
    (0..cell.width())
        .rev()
        .find(|&x| {
            (0..cell.height()).any(|y| {
                let at = (cell.min.y + y) as usize * width as usize + (cell.min.x + x) as usize;
                pixels.get(at * 4 + 3).is_some_and(|alpha| *alpha != 0)
            })
        })
        .map_or(0, |x| x + 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pack(default: &str) -> ResourceSet {
        [(
            "minecraft/font/default.json".to_owned(),
            default.as_bytes().to_vec(),
        )]
        .into_iter()
        .collect()
    }

    #[test]
    fn a_provider_of_an_unknown_kind_is_skipped() {
        let pack = pack(
            r#"{"providers":[{"type":"ttf","file":"x.ttf"},{"type":"space","advances":{"a":3}}]}"#,
        );
        let sources = sources(&pack, "minecraft:default").unwrap();
        assert_eq!(sources.len(), 1);
        assert!(matches!(sources[0].kind, Kind::Space(_)));
    }

    #[test]
    fn a_file_with_an_invalid_provider_is_ignored_as_a_whole() {
        let pack = pack(
            r#"{"providers":[{"type":"space","advances":{"a":3}},{"type":"space","advances":"no"}]}"#,
        );
        assert!(sources(&pack, "minecraft:default").unwrap().is_empty());
        for definition in [
            "not json",
            r#"{"providers":[{"type":"space","advances":{"ab":3}}]}"#,
            r#"{"providers":[{"type":"bitmap","file":"minecraft:font/ascii.png","height":8,"ascent":9,"chars":["a"]}]}"#,
            r#"{"providers":[{"type":"bitmap","file":"minecraft:font/ascii.png","ascent":7,"chars":["ab","c"]}]}"#,
            r#"{"providers":[{"type":"unihex","hex_file":"minecraft:font/unifont.zip","size_overrides":[{"from":"b","to":"a","left":0,"right":1}]}]}"#,
        ] {
            assert!(
                sources(&self::pack(definition), "minecraft:default")
                    .unwrap()
                    .is_empty(),
                "{definition}"
            );
        }
    }
}
