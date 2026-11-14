use std::collections::{BTreeMap, HashMap};

use bevy::log::warn;

use mcrs_minecraft_model::atlas::decode_png;
use mcrs_minecraft_model::resource_set::ResourceSet;
use mcrs_minecraft_model_file::resource_id::{full_id, split_id};
use serde::de::Error as _;
use serde::{Deserialize, Deserializer};

use crate::font_bitmap::{BitmapPage, BitmapSpec};
use crate::font_unihex::{Unihex, UnihexSpec};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Toggle {
    #[default]
    Off,
    On,
}

/// The switches a player controls: "Force Unicode Font" and "Japanese Glyph Variants".
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FontOptions {
    pub uniform: Toggle,
    pub jp: Toggle,
}

/// The options a provider needs on or off; a missing entry passes always.
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

    /// Each entry of `over` replaces the entry of the same name.
    fn overridden_by(self, over: Self) -> Self {
        Self {
            uniform: over.uniform.or(self.uniform),
            jp: over.jp.or(self.jp),
        }
    }
}

#[derive(Debug)]
pub(crate) enum Kind {
    Space(BTreeMap<char, f32>),
    Bitmap(BitmapPage),
    Unihex(Unihex),
}

#[derive(Debug)]
pub(crate) struct Source {
    pub(crate) filter: Filter,
    pub(crate) kind: Kind,
}

#[derive(Deserialize)]
struct Definition {
    providers: Vec<Provider>,
}

impl Definition {
    /// The field rules that serde cannot state.
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
    /// A kind this client does not read (`ttf`, or one it has never heard of): skipped.
    #[serde(other)]
    Other,
}

#[derive(Deserialize)]
struct SpaceSpec {
    #[serde(default)]
    filter: Filter,
    advances: HashMap<CodePoint, f32>,
}

/// A string of exactly one code point.
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

/// The glyph sources of the font `id`, with each reference replaced by the sources it names.
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
    // FONT-DEF-001, step 4: a file that is not valid JSON, or whose provider has an invalid field,
    // is ignored as a whole with a warning.
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
                sources.push(Source {
                    filter: spec.filter,
                    kind: Kind::Unihex(
                        Unihex::new(&spec, bytes).map_err(|error| format!("{archive}: {error}"))?,
                    ),
                });
            }
        }
    }
    loading.pop();
    Ok(sources)
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
