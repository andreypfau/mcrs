use std::collections::{BTreeMap, HashMap};

use mcrs_minecraft_model::atlas::decode_png;
use mcrs_minecraft_model::resource_set::ResourceSet;
use mcrs_minecraft_model_file::resource_id::{full_id, split_id};
use serde::Deserialize;

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

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Provider {
    Bitmap(BitmapSpec),
    Space(SpaceSpec),
    Unihex(UnihexSpec),
    Reference(ReferenceSpec),
}

#[derive(Deserialize)]
struct SpaceSpec {
    #[serde(default)]
    filter: Filter,
    advances: HashMap<String, f32>,
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
    let definition: Definition = serde_json::from_slice(
        pack.read(&file)
            .map_err(|error| format!("the font {id}: {error}"))?,
    )
    .map_err(|error| format!("{file}: {error}"))?;
    let mut sources = Vec::new();
    for provider in definition.providers {
        match provider {
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
                    .map(|(key, advance)| {
                        let mut chars = key.chars();
                        match (chars.next(), chars.next()) {
                            (Some(ch), None) => Ok((ch, *advance)),
                            _ => Err(format!(
                                "{file}: the space key {key:?} is not one code point"
                            )),
                        }
                    })
                    .collect::<Result<_, String>>()?;
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
