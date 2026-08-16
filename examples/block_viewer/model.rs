//! Resource-pack loading: blockstate variant selection and block-model parent resolution.
//!
//! Mirrors `net.minecraft.client.resources.model.ModelBakery` / `BlockStateModelLoader`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;

pub fn assets_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("assets")
}

/// `block/cube` and `minecraft:block/cube` are the same resource; the vanilla pack mixes both
/// forms inside a single parent chain, so every identifier is normalised before any lookup.
fn split_id(id: &str) -> (&str, &str) {
    id.split_once(':').unwrap_or(("minecraft", id))
}

pub fn resource_path(id: &str, kind: &str, ext: &str) -> PathBuf {
    let (namespace, path) = split_id(id);
    assets_dir()
        .join(namespace)
        .join(kind)
        .join(format!("{path}.{ext}"))
}

#[derive(Debug, Deserialize)]
pub struct BlockStateFile {
    #[serde(default)]
    variants: HashMap<String, VariantValue>,
    #[serde(default)]
    multipart: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum VariantValue {
    Many(Vec<Variant>),
    One(Variant),
}

#[derive(Debug, Clone, Deserialize)]
pub struct Variant {
    pub model: String,
    #[serde(default)]
    pub x: i32,
    #[serde(default)]
    pub y: i32,
    #[serde(default)]
    pub z: i32,
    #[serde(default)]
    pub uvlock: bool,
}

impl BlockStateFile {
    pub fn load(block: &str) -> Result<Self, String> {
        let path = resource_path(block, "blockstates", "json");
        let text = std::fs::read_to_string(&path)
            .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        serde_json::from_str(&text).map_err(|e| format!("cannot parse {}: {e}", path.display()))
    }

    pub fn select(&self, props: &[(&str, &str)]) -> Result<Variant, String> {
        if !self.multipart.is_empty() && self.variants.is_empty() {
            return Err("multipart blockstates are not supported".into());
        }
        let mut keys: Vec<&str> = self.variants.keys().map(String::as_str).collect();
        keys.sort_unstable();
        for key in &keys {
            if key_matches(key, props) {
                // A weighted list is picked by a position-seeded random in the game; taking the
                // first entry keeps a single block deterministic across rebakes.
                return match &self.variants[*key] {
                    VariantValue::One(v) => Ok(v.clone()),
                    VariantValue::Many(v) => v
                        .first()
                        .cloned()
                        .ok_or_else(|| format!("empty variant list for `{key}`")),
                };
            }
        }
        Err(format!(
            "no variant matches {props:?}; this blockstate offers: {}",
            keys.iter()
                .map(|key| if key.is_empty() {
                    "<no properties>"
                } else {
                    key
                })
                .collect::<Vec<_>>()
                .join(" | ")
        ))
    }
}

fn key_matches(key: &str, props: &[(&str, &str)]) -> bool {
    if key.is_empty() {
        return true;
    }
    key.split(',').all(|constraint| {
        let Some((name, value)) = constraint.split_once('=') else {
            return false;
        };
        props.iter().any(|(k, v)| *k == name && *v == value)
    })
}

#[derive(Debug, Clone, Deserialize)]
pub struct Element {
    pub from: [f32; 3],
    pub to: [f32; 3],
    #[serde(default)]
    pub rotation: Option<ElementRotation>,
    #[serde(default = "yes")]
    pub shade: bool,
    #[serde(default)]
    pub faces: HashMap<String, Face>,
}

fn yes() -> bool {
    true
}

#[derive(Debug, Clone, Deserialize)]
pub struct ElementRotation {
    pub origin: [f32; 3],
    pub axis: String,
    pub angle: f32,
    #[serde(default)]
    pub rescale: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Face {
    pub texture: String,
    #[serde(default)]
    pub uv: Option<[f32; 4]>,
    #[serde(default)]
    pub cullface: Option<String>,
    #[serde(default)]
    pub rotation: i32,
}

/// A texture slot is usually a plain sprite id, but 110 vanilla models spell it out as an object
/// carrying render hints alongside the sprite.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum TextureValue {
    Sprite(String),
    Detailed { sprite: String },
}

impl TextureValue {
    fn sprite(&self) -> &str {
        match self {
            TextureValue::Sprite(sprite) | TextureValue::Detailed { sprite } => sprite,
        }
    }
}

#[derive(Debug, Deserialize)]
struct RawModel {
    parent: Option<String>,
    #[serde(default)]
    textures: HashMap<String, TextureValue>,
    elements: Option<Vec<Element>>,
    #[serde(rename = "ambientocclusion")]
    ambient_occlusion: Option<bool>,
}

#[derive(Debug)]
pub struct ResolvedModel {
    pub elements: Vec<Element>,
    pub textures: HashMap<String, String>,
    pub ambient_occlusion: bool,
}

impl ResolvedModel {
    /// A face's `texture` is either a concrete sprite id or a `#slot` reference into the merged
    /// texture map. Unresolvable references are an error, matching vanilla's missing-texture path.
    pub fn sprite_of<'a>(&'a self, face: &'a Face) -> Result<&'a str, String> {
        let key = face.texture.strip_prefix('#').unwrap_or(&face.texture);
        match self.textures.get(key) {
            Some(v) if !v.starts_with('#') => Ok(v),
            _ if !face.texture.starts_with('#') => Ok(&face.texture),
            _ => Err(format!("unresolved texture slot `{}`", face.texture)),
        }
    }
}

pub fn resolve_model(id: &str) -> Result<ResolvedModel, String> {
    let mut chain: Vec<RawModel> = Vec::new();
    let mut next = Some(id.to_string());
    while let Some(current) = next {
        let path = resource_path(&current, "models", "json");
        let text = std::fs::read_to_string(&path)
            .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        let raw: RawModel = serde_json::from_str(&text)
            .map_err(|e| format!("cannot parse {}: {e}", path.display()))?;
        next = raw.parent.clone();
        chain.push(raw);
        if chain.len() > 16 {
            return Err(format!("model parent chain of `{id}` is too deep"));
        }
    }

    // `elements` is inherited whole from the nearest ancestor that declares it; it never merges.
    let elements = chain
        .iter()
        .find_map(|m| m.elements.clone())
        .unwrap_or_default();
    let ambient_occlusion = chain
        .iter()
        .find_map(|m| m.ambient_occlusion)
        .unwrap_or(true);

    // `textures` does merge, child over parent, and only then are `#refs` resolved: `cube_column`
    // points `down` at `#end` but leaves `end` to its child, so per-level resolution would fail.
    let mut textures: HashMap<String, String> = HashMap::new();
    for model in chain.iter().rev() {
        textures.extend(
            model
                .textures
                .iter()
                .map(|(k, v)| (k.clone(), v.sprite().to_string())),
        );
    }
    loop {
        let snapshot = textures.clone();
        let mut changed = false;
        for value in textures.values_mut() {
            if let Some(slot) = value.strip_prefix('#')
                && let Some(target) = snapshot.get(slot)
                && !target.starts_with('#')
            {
                *value = target.clone();
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }

    Ok(ResolvedModel {
        elements,
        textures,
        ambient_occlusion,
    })
}
