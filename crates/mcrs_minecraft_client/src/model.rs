//! Resource-pack loading: blockstate variant selection and block-model parent resolution.
//!
//! Mirrors `net.minecraft.client.resources.model.ModelBakery` / `BlockStateModelLoader`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use bevy::asset::io::{AssetSourceId, ErasedAssetReader};
use bevy::math::Vec3;
use bevy::prelude::{AssetServer, Resource};
use bevy::tasks::futures_lite::StreamExt;
use mcrs_minecraft_assets::asset::read_whole;
use serde::{Deserialize, Serialize};

use crate::vanilla;

/// The folders of the resource pack the renderer draws from. Everything under them is held in
/// memory, because a block state first seen mid-stream has to bake without an await.
const RESOURCE_FOLDERS: [&str; 7] = [
    "blockstates",
    "models",
    "textures",
    "items",
    "atlases",
    "font",
    "lang",
];

/// Biome tints come from the data pack, whose `beta_*` biomes exist only in the repo.
const DATA_FOLDERS: [&str; 1] = ["worldgen/biome"];

pub fn is_resource(path: &str) -> bool {
    path.split_once('/').is_some_and(|(_, rest)| {
        RESOURCE_FOLDERS.iter().any(|folder| {
            rest.strip_prefix(folder)
                .is_some_and(|tail| tail.starts_with('/'))
        })
    })
}

/// The resource pack, read once through the asset system and thereafter immutable.
#[derive(Resource, Default)]
pub struct Pack {
    files: HashMap<String, Vec<u8>>,
}

impl Pack {
    pub async fn load(assets: &AssetServer) -> Result<Self, String> {
        let mut files = HashMap::new();
        for (source, folders) in [
            (AssetSourceId::from(vanilla::SOURCE), &RESOURCE_FOLDERS[..]),
            (AssetSourceId::Default, &DATA_FOLDERS[..]),
        ] {
            let reader = assets
                .get_source(source.clone())
                .map_err(|error| format!("no {source} asset source: {error}"))?;
            Self::walk(reader.reader(), folders, &mut files).await?;
        }
        Ok(Self { files })
    }

    async fn walk(
        reader: &dyn ErasedAssetReader,
        folders: &[&str],
        files: &mut HashMap<String, Vec<u8>>,
    ) -> Result<(), String> {
        let mut namespaces = reader
            .read_directory(Path::new(""))
            .await
            .map_err(|error| format!("cannot list the asset root: {error}"))?;
        let mut pending: Vec<PathBuf> = Vec::new();
        while let Some(namespace) = namespaces.next().await {
            pending.extend(folders.iter().map(|folder| namespace.join(folder)));
        }

        while let Some(directory) = pending.pop() {
            let Ok(mut entries) = reader.read_directory(&directory).await else {
                continue;
            };
            while let Some(path) = entries.next().await {
                if reader.is_directory(&path).await.unwrap_or(false) {
                    pending.push(path);
                    continue;
                }
                let bytes = read_whole(reader, &path)
                    .await
                    .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
                files.insert(path.to_string_lossy().into_owned(), bytes);
            }
        }
        Ok(())
    }

    pub fn len(&self) -> usize {
        self.files.len()
    }

    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    pub fn read(&self, path: &str) -> Result<&[u8], String> {
        self.get(path)
            .ok_or_else(|| format!("{path} is not in the resource pack"))
    }

    pub fn get(&self, path: &str) -> Option<&[u8]> {
        self.files.get(path).map(Vec::as_slice)
    }

    /// Every file under `<namespace>/<folder>/`, as `(id, bytes)` with the id in
    /// `namespace:path` form and the extension dropped.
    pub fn entries<'a>(
        &'a self,
        folder: &'a str,
        ext: &'a str,
    ) -> impl Iterator<Item = (String, &'a [u8])> + 'a {
        let suffix = format!(".{ext}");
        self.files.iter().filter_map(move |(path, bytes)| {
            let (namespace, rest) = path.split_once('/')?;
            let rest = rest.strip_prefix(folder)?.strip_prefix('/')?;
            let id = rest.strip_suffix(suffix.as_str())?;
            Some((format!("{namespace}:{id}"), bytes.as_slice()))
        })
    }
}

impl FromIterator<(String, Vec<u8>)> for Pack {
    fn from_iter<I: IntoIterator<Item = (String, Vec<u8>)>>(files: I) -> Self {
        Self {
            files: files.into_iter().collect(),
        }
    }
}

#[cfg(test)]
impl Pack {
    /// The client jar's resource pack plus the repo's data folders, read without an asset
    /// system: a test has none to read them through.
    pub fn corpus() -> &'static Pack {
        static CORPUS: std::sync::LazyLock<Pack> = std::sync::LazyLock::new(|| {
            let root = crate::asset_corpus();
            let mut files: HashMap<String, Vec<u8>> =
                vanilla::resource_files().iter().cloned().collect();
            let mut pending: Vec<PathBuf> = std::fs::read_dir(&root)
                .expect("the corpus is next to the workspace")
                .filter_map(|entry| entry.ok())
                .flat_map(|namespace| {
                    DATA_FOLDERS
                        .iter()
                        .map(move |folder| namespace.path().join(folder))
                })
                .collect();
            while let Some(directory) = pending.pop() {
                let Ok(entries) = std::fs::read_dir(&directory) else {
                    continue;
                };
                for entry in entries.filter_map(|entry| entry.ok()) {
                    let path = entry.path();
                    if path.is_dir() {
                        pending.push(path);
                        continue;
                    }
                    let relative = path.strip_prefix(&root).expect("walked from the root");
                    let bytes = std::fs::read(&path).expect("the corpus is readable");
                    files.insert(relative.to_string_lossy().into_owned(), bytes);
                }
            }
            Pack { files }
        });
        &CORPUS
    }
}

/// `block/cube` and `minecraft:block/cube` are the same resource; the vanilla pack mixes both
/// forms inside a single parent chain, so every identifier is normalised before any lookup.
pub fn split_id(id: &str) -> (&str, &str) {
    id.split_once(':').unwrap_or(("minecraft", id))
}

pub fn resource_path(id: &str, kind: &str, ext: &str) -> String {
    let (namespace, path) = split_id(id);
    format!("{namespace}/{kind}/{path}.{ext}")
}

#[derive(Debug, Deserialize)]
pub struct BlockStateFile {
    #[serde(default)]
    variants: HashMap<String, VariantValue>,
    #[serde(default)]
    multipart: Vec<MultipartCase>,
}

#[derive(Debug, Clone, Deserialize)]
struct MultipartCase {
    #[serde(default)]
    when: Option<Condition>,
    apply: VariantValue,
}

/// A `when` clause is either a set of property constraints joined by AND, or an explicit
/// `OR` / `AND` list of such sets. Constraint values may be `a|b|c` alternatives.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum Condition {
    Or {
        #[serde(rename = "OR")]
        or: Vec<Condition>,
    },
    And {
        #[serde(rename = "AND")]
        and: Vec<Condition>,
    },
    Terms(HashMap<String, Term>),
}

/// A property value spelled as the blockstate JSON spells it: quoted for a string, bare for the
/// booleans and integers, which still name a string-valued property.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
enum Term {
    Flag(bool),
    Int(i64),
    Text(String),
}

impl std::fmt::Display for Term {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Term::Flag(flag) => write!(f, "{flag}"),
            Term::Int(int) => write!(f, "{int}"),
            Term::Text(text) => f.write_str(text),
        }
    }
}

impl Condition {
    fn matches(&self, props: &[(&str, &str)]) -> bool {
        match self {
            Condition::Or { or } => or.iter().any(|c| c.matches(props)),
            Condition::And { and } => and.iter().all(|c| c.matches(props)),
            Condition::Terms(terms) => terms.iter().all(|(name, expected)| {
                let expected = expected.to_string();
                let actual = props.iter().find(|(k, _)| k == name).map(|(_, v)| *v);
                match actual {
                    Some(actual) => expected.split('|').any(|alt| alt == actual),
                    None => false,
                }
            }),
        }
    }
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
    pub fn is_multipart(&self) -> bool {
        !self.multipart.is_empty() && self.variants.is_empty()
    }

    /// Every variant that contributes geometry to this state. A `variants` blockstate contributes
    /// exactly one; a `multipart` blockstate contributes every case whose `when` clause matches.
    pub fn select_all(&self, props: &[(&str, &str)]) -> Result<Vec<Variant>, String> {
        if !self.is_multipart() {
            return Ok(vec![self.select(props)?]);
        }
        let mut out = Vec::new();
        for case in &self.multipart {
            if case.when.as_ref().is_some_and(|w| !w.matches(props)) {
                continue;
            }
            match &case.apply {
                VariantValue::One(v) => out.push(v.clone()),
                VariantValue::Many(v) => {
                    if let Some(first) = v.first() {
                        out.push(first.clone());
                    }
                }
            }
        }
        if out.is_empty() {
            return Err(format!("no multipart case matches {props:?}"));
        }
        Ok(out)
    }

    pub fn load(pack: &Pack, block: &str) -> Result<Self, String> {
        let path = resource_path(block, "blockstates", "json");
        serde_json::from_slice(pack.read(&path)?)
            .map_err(|error| format!("cannot parse {path}: {error}"))
    }

    pub fn select(&self, props: &[(&str, &str)]) -> Result<Variant, String> {
        if self.is_multipart() {
            return self
                .select_all(props)?
                .into_iter()
                .next()
                .ok_or_else(|| "no multipart case matches".to_string());
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
    /// Slot into the block's tint palette (grass, foliage, water). Absent means untinted.
    #[serde(rename = "tintindex", default)]
    pub tint_index: Option<u32>,
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
    #[serde(default)]
    display: Option<HashMap<String, RawItemTransform>>,
    #[serde(default)]
    gui_light: Option<GuiLight>,
}

#[derive(Debug, Deserialize)]
struct RawItemTransform {
    #[serde(default)]
    rotation: [f32; 3],
    #[serde(default)]
    translation: [f32; 3],
    #[serde(default = "ones")]
    scale: [f32; 3],
}

fn ones() -> [f32; 3] {
    [1.0; 3]
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ItemTransform {
    pub rotation_deg: Vec3,
    pub translation: Vec3,
    pub scale: Vec3,
}

impl ItemTransform {
    pub const NONE: Self = Self {
        rotation_deg: Vec3::ZERO,
        translation: Vec3::ZERO,
        scale: Vec3::ONE,
    };

    fn from_raw(raw: &RawItemTransform) -> Self {
        Self {
            rotation_deg: Vec3::from(raw.rotation),
            translation: (Vec3::from(raw.translation) / 16.0)
                .clamp(Vec3::splat(-5.0), Vec3::splat(5.0)),
            scale: Vec3::from(raw.scale).clamp(Vec3::splat(-4.0), Vec3::splat(4.0)),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GuiLight {
    Front,
    Side,
}

pub const GENERATED_ITEM_MODEL_ID: &str = "minecraft:builtin/generated";

#[derive(Debug)]
pub struct ResolvedModel {
    pub elements: Vec<Element>,
    pub textures: HashMap<String, String>,
    pub ambient_occlusion: bool,
    pub display: ItemTransform,
    pub gui_light: GuiLight,
    /// The parent chain ends at `builtin/generated`: the geometry is extruded from the layers.
    pub generated: bool,
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

pub fn resolve_model(pack: &Pack, id: &str) -> Result<ResolvedModel, String> {
    let mut chain: Vec<RawModel> = Vec::new();
    let mut next = Some(id.to_string());
    let mut generated = false;
    while let Some(current) = next {
        if split_id(&current) == split_id(GENERATED_ITEM_MODEL_ID) {
            generated = true;
            break;
        }
        let path = resource_path(&current, "models", "json");
        let raw: RawModel = serde_json::from_slice(pack.read(&path)?)
            .map_err(|error| format!("cannot parse {path}: {error}"))?;
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
    let gui_light = chain
        .iter()
        .find_map(|m| m.gui_light)
        .unwrap_or(if generated {
            GuiLight::Front
        } else {
            GuiLight::Side
        });
    let display = chain
        .iter()
        .find_map(|m| m.display.as_ref()?.get("gui"))
        .map_or(ItemTransform::NONE, ItemTransform::from_raw);

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
        display,
        gui_light,
        generated,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_asset_system_reads_the_pack_the_corpus_holds() {
        let mut app = bevy::app::App::new();
        let root = bevy::asset::io::memory::Dir::default();
        vanilla::fill(&root, vanilla::resource_files().clone());
        vanilla::register_source(&mut app, root);
        app.add_plugins(bevy::MinimalPlugins)
            .add_plugins(bevy::asset::AssetPlugin {
                file_path: crate::asset_corpus().to_string_lossy().into_owned(),
                ..Default::default()
            });
        let pack = bevy::tasks::block_on(Pack::load(app.world().resource::<AssetServer>()))
            .expect("the corpus reads through the asset system");

        assert_eq!(pack.len(), Pack::corpus().len());
        let stone = resource_path("minecraft:block/stone", "textures", "png");
        assert_eq!(
            pack.read(&stone).unwrap(),
            Pack::corpus().read(&stone).unwrap(),
        );
    }

    #[test]
    fn a_condition_term_round_trips_through_every_spelling() {
        for spelling in [r#""north|east""#, "true", "false", "3", "-1"] {
            let term: Term = serde_json::from_str(spelling).expect("a term parses");
            assert_eq!(serde_json::to_string(&term).unwrap(), spelling);
        }
        assert!(matches!(
            serde_json::from_str::<Term>("true").unwrap(),
            Term::Flag(true)
        ));
        assert!(matches!(
            serde_json::from_str::<Term>("3").unwrap(),
            Term::Int(3)
        ));
    }

    #[test]
    fn a_condition_matches_alternatives_and_bare_values() {
        let when: Condition =
            serde_json::from_str(r#"{"facing": "north|east", "powered": true, "delay": 3}"#)
                .expect("a when clause parses");

        assert!(when.matches(&[("facing", "east"), ("powered", "true"), ("delay", "3")]));
        assert!(when.matches(&[("facing", "north"), ("powered", "true"), ("delay", "3")]));
        assert!(!when.matches(&[("facing", "south"), ("powered", "true"), ("delay", "3")]));
        assert!(!when.matches(&[("facing", "north"), ("powered", "false"), ("delay", "3")]));
        assert!(!when.matches(&[("facing", "north"), ("powered", "true"), ("delay", "4")]));
        assert!(!when.matches(&[
            ("facing", "north|east"),
            ("powered", "true"),
            ("delay", "3")
        ]));
    }

    #[test]
    fn a_generated_item_is_front_lit_with_no_gui_transform() {
        let model = resolve_model(Pack::corpus(), "minecraft:item/generated").unwrap();
        assert!(model.generated);
        assert_eq!(model.gui_light, GuiLight::Front);
        assert_eq!(model.display, ItemTransform::NONE);
        let stick = resolve_model(Pack::corpus(), "minecraft:item/stick").unwrap();
        assert!(stick.generated);
        assert_eq!(stick.textures["layer0"], "minecraft:item/stick");
    }

    #[test]
    fn a_block_item_is_side_lit_with_the_gui_rotation_of_block_block() {
        let model = resolve_model(Pack::corpus(), "minecraft:block/stone").unwrap();
        assert!(!model.generated);
        assert_eq!(model.gui_light, GuiLight::Side);
        let gui = model.display;
        assert_eq!(gui.rotation_deg, Vec3::new(30.0, 225.0, 0.0));
        assert_eq!(gui.scale, Vec3::splat(0.625));
        assert_eq!(gui.translation, Vec3::ZERO);
    }

    #[test]
    fn a_declared_gui_transform_wins_over_the_parents() {
        let mut pack = Pack::default();
        let put = |pack: &mut Pack, id: &str, json: &str| {
            pack.files.insert(
                resource_path(id, "models", "json"),
                json.as_bytes().to_vec(),
            );
        };
        put(
            &mut pack,
            "minecraft:block/parent",
            r#"{"display": {"gui": {"rotation": [1, 2, 3]}, "ground": {"scale": [0.5, 0.5, 0.5]}}}"#,
        );
        put(
            &mut pack,
            "minecraft:block/child",
            r#"{"parent": "block/parent", "display": {"gui": {"translation": [16, 0, 0]}}}"#,
        );
        put(
            &mut pack,
            "minecraft:block/blank",
            r#"{"parent": "block/parent", "display": {"gui": {}}}"#,
        );
        let child = resolve_model(&pack, "minecraft:block/child").unwrap();
        assert_eq!(child.display.translation, Vec3::new(1.0, 0.0, 0.0));
        assert_eq!(child.display.rotation_deg, Vec3::ZERO);
        let blank = resolve_model(&pack, "minecraft:block/blank").unwrap();
        assert_eq!(blank.display, ItemTransform::NONE);
    }

    #[test]
    fn a_multipart_fence_picks_up_its_connected_sides() {
        let states = BlockStateFile::load(Pack::corpus(), "minecraft:oak_fence")
            .expect("the fence blockstate is in the corpus");
        assert!(states.is_multipart());

        assert_eq!(states.select_all(&[]).unwrap().len(), 1);
        assert_eq!(
            states
                .select_all(&[("north", "true"), ("east", "true")])
                .unwrap()
                .len(),
            3
        );
    }
}
