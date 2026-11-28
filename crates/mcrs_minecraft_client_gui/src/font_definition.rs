use std::collections::{BTreeMap, HashMap};

use bevy::math::{Rect, URect, UVec2};

use mcrs_minecraft_resource::ResourceLocation;
use mcrs_minecraft_resource_pack::{PackFiles, RgbaImage};
use serde::de::Error as _;
use serde::{Deserialize, Deserializer};

use crate::definition_error::DefinitionError;
use crate::font_error::FontError;
use crate::font_glyph::{Glyph, GlyphImage, ImageSource};
use crate::font_notice::FontNotice;
use crate::font_unihex::{SizeOverride, Unihex};
use crate::inflated_file::InflatedFile;
use crate::unihex_error::UnihexError;

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
    fn check(&self) -> Result<(), DefinitionError> {
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
    fn check(&self) -> Result<(), DefinitionError> {
        match self
            .size_overrides
            .iter()
            .find(|size| size.from.0 >= size.to.0)
        {
            Some(size) => Err(DefinitionError::UnorderedOverride {
                hex_file: self.hex_file.clone(),
                from: size.from.0,
                to: size.to.0,
            }),
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

pub(crate) struct Loading<'a, F> {
    pack: &'a PackFiles,
    inflate: F,
    stack: Vec<String>,
    notices: Vec<FontNotice>,
}

impl<'a, F: Fn(&[u8]) -> Result<Vec<InflatedFile>, String>> Loading<'a, F> {
    pub(crate) fn new(pack: &'a PackFiles, inflate: F) -> Self {
        Self {
            pack,
            inflate,
            stack: Vec::new(),
            notices: Vec::new(),
        }
    }

    pub(crate) fn into_notices(self) -> Vec<FontNotice> {
        self.notices
    }

    pub(crate) fn sources(&mut self, id: &str) -> Result<Vec<Source>, FontError> {
        let location = parse_id(id)?;
        let id = format!("{}:{}", location.namespace(), location.path());
        if self.stack.contains(&id) {
            return Err(FontError::SelfReference { id });
        }
        let file = format!("{}/font/{}.json", location.namespace(), location.path());
        let bytes = self.pack.read(&file)?;
        let definition = match serde_json::from_slice::<Definition>(bytes)
            .map_err(|error| DefinitionError::Json {
                reason: error.to_string(),
            })
            .and_then(|definition| definition.check().map(|()| definition))
        {
            Ok(definition) => definition,
            Err(reason) => {
                self.notices.push(FontNotice::IgnoredFile { file, reason });
                return Ok(Vec::new());
            }
        };
        self.stack.push(id);
        let sources = self.providers(&file, definition.providers);
        self.stack.pop();
        sources
    }

    fn providers(
        &mut self,
        file: &str,
        providers: Vec<Provider>,
    ) -> Result<Vec<Source>, FontError> {
        let mut sources = Vec::new();
        for provider in providers {
            match provider {
                Provider::Other => self.notices.push(FontNotice::UnknownProvider {
                    file: file.to_owned(),
                }),
                Provider::Reference(reference) => {
                    for mut source in self.sources(&reference.id)? {
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
                Provider::Bitmap(spec) => sources.push(Source {
                    filter: spec.filter,
                    kind: Kind::Bitmap(self.bitmap(&spec)?),
                }),
                Provider::Unihex(spec) => sources.push(Source {
                    filter: spec.filter,
                    kind: Kind::Unihex(self.unihex(&spec)?),
                }),
            }
        }
        Ok(sources)
    }

    fn bitmap(&self, spec: &BitmapSpec) -> Result<BitmapPage, FontError> {
        let location = parse_id(&spec.file)?;
        let texture = format!("{}/textures/{}", location.namespace(), location.path());
        let image = RgbaImage::decode_png(self.pack.read(&texture)?)
            .map_err(|error| FontError::Png { texture, error })?;
        BitmapPage::new(spec, image.rgba, image.width, image.height)
    }

    fn unihex(&self, spec: &UnihexSpec) -> Result<Unihex, FontError> {
        let location = parse_id(&spec.hex_file)?;
        let archive = format!("{}/{}", location.namespace(), location.path());
        let files =
            (self.inflate)(self.pack.read(&archive)?).map_err(|reason| FontError::Unihex {
                archive: archive.clone(),
                error: UnihexError::Inflate { reason },
            })?;
        Unihex::new(&files, &spec.overrides()).map_err(|error| FontError::Unihex { archive, error })
    }
}

fn parse_id(id: &str) -> Result<ResourceLocation<'_>, FontError> {
    ResourceLocation::read(id).map_err(|error| FontError::InvalidId {
        id: id.to_owned(),
        reason: error.reason,
    })
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
    pub(crate) fn check(&self) -> Result<(), DefinitionError> {
        if self.ascent > self.height {
            return Err(DefinitionError::AscentAboveHeight {
                texture: self.file.clone(),
                ascent: self.ascent,
                height: self.height,
            });
        }
        let columns = self.chars.first().map_or(0, |row| row.chars().count());
        if columns == 0 || self.chars.iter().any(|row| row.chars().count() != columns) {
            return Err(DefinitionError::UnevenRows {
                texture: self.file.clone(),
            });
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
    ) -> Result<Self, FontError> {
        let columns = spec.chars.first().map_or(0, |row| row.chars().count()) as u32;
        let rows = spec.chars.len() as u32;
        let (cell_width, cell_height) = (width / columns, height / rows);
        if cell_width == 0 || cell_height == 0 || spec.height <= 0 {
            return Err(FontError::Grid {
                texture: spec.file.clone(),
            });
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

    fn pack(default: &str) -> PackFiles {
        let mut pack = PackFiles::default();
        pack.add_file("minecraft/font/default.json", default.as_bytes().to_vec())
            .unwrap();
        pack
    }

    fn load(pack: &PackFiles) -> (Vec<Source>, Vec<FontNotice>) {
        let mut loading = Loading::new(pack, |_: &[u8]| Ok(Vec::new()));
        let sources = loading.sources("minecraft:default").unwrap();
        (sources, loading.into_notices())
    }

    #[test]
    fn a_provider_of_an_unknown_kind_is_skipped() {
        let pack = pack(
            r#"{"providers":[{"type":"ttf","file":"x.ttf"},{"type":"space","advances":{"a":3}}]}"#,
        );
        let (sources, notices) = load(&pack);
        assert_eq!(sources.len(), 1);
        assert!(matches!(sources[0].kind, Kind::Space(_)));
        assert_eq!(
            notices,
            [FontNotice::UnknownProvider {
                file: "minecraft/font/default.json".to_owned()
            }]
        );
    }

    #[test]
    fn a_file_with_an_invalid_provider_is_ignored_as_a_whole() {
        for definition in [
            r#"{"providers":[{"type":"space","advances":{"a":3}},{"type":"space","advances":"no"}]}"#,
            "not json",
            r#"{"providers":[{"type":"space","advances":{"ab":3}}]}"#,
            r#"{"providers":[{"type":"bitmap","file":"minecraft:font/ascii.png","height":8,"ascent":9,"chars":["a"]}]}"#,
            r#"{"providers":[{"type":"bitmap","file":"minecraft:font/ascii.png","ascent":7,"chars":["ab","c"]}]}"#,
            r#"{"providers":[{"type":"unihex","hex_file":"minecraft:font/unifont.zip","size_overrides":[{"from":"b","to":"a","left":0,"right":1}]}]}"#,
        ] {
            let (sources, notices) = load(&pack(definition));
            assert!(sources.is_empty(), "{definition}");
            assert!(
                matches!(
                    notices.as_slice(),
                    [FontNotice::IgnoredFile { file, .. }] if file == "minecraft/font/default.json"
                ),
                "{definition}: {notices:?}"
            );
        }
    }
}
