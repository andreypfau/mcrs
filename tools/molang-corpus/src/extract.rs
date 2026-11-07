use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Context as _;

use crate::json::{self, Json, JsonError};

/// The JSON form of one Molang field value.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ValueForm {
    String,
    Number,
    Bool,
    /// An array of strings that the loader joins into one expression (MOL-LOAD-002).
    ScriptArray,
    /// An object `{"expression": …, "version": …}`.
    Versioned,
}

impl ValueForm {
    pub fn name(self) -> &'static str {
        match self {
            ValueForm::String => "string",
            ValueForm::Number => "number",
            ValueForm::Bool => "bool",
            ValueForm::ScriptArray => "script_array",
            ValueForm::Versioned => "versioned_object",
        }
    }
}

/// One Molang field value of a pack file.
#[derive(Clone, Debug, PartialEq)]
pub struct Occurrence {
    pub text: String,
    pub form: ValueForm,
    /// The entries of a script array, before the join.
    pub entries: Vec<String>,
    pub version: Option<String>,
    pub kind: &'static str,
    pub field: String,
    pub file: String,
    pub pointer: String,
    pub format_version: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PackFile {
    pub kind: &'static str,
    pub format_version: String,
}

#[derive(Clone, Debug)]
pub struct BrokenFile {
    pub file: String,
    pub error: JsonError,
}

#[derive(Clone, Debug)]
pub struct Manifest {
    pub file: String,
    pub min_engine_version: String,
}

#[derive(Clone, Debug, Default)]
pub struct Extraction {
    pub occurrences: Vec<Occurrence>,
    pub files: Vec<PackFile>,
    pub broken: Vec<BrokenFile>,
    pub manifests: Vec<Manifest>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Side {
    Resource,
    Behavior,
}

/// Walks a pack, or a directory with `resource_pack` and `behavior_pack`.
pub fn extract(root: &Path) -> anyhow::Result<Extraction> {
    let mut extraction = Extraction::default();
    let resource = root.join("resource_pack");
    let behavior = root.join("behavior_pack");
    if resource.is_dir() || behavior.is_dir() {
        for (pack, side) in [(resource, Side::Resource), (behavior, Side::Behavior)] {
            if pack.is_dir() {
                walk_pack(root, &pack, side, &mut extraction)?;
            }
        }
    } else {
        let side = if root.join("entities").is_dir() {
            Side::Behavior
        } else {
            Side::Resource
        };
        walk_pack(root, root, side, &mut extraction)?;
    }
    Ok(extraction)
}

fn walk_pack(root: &Path, pack: &Path, side: Side, out: &mut Extraction) -> anyhow::Result<()> {
    let mut files = Vec::new();
    collect_json(pack, &mut files)?;
    files.sort();
    for path in files {
        let relative = path.strip_prefix(root).unwrap_or(&path);
        let file = relative.to_string_lossy().replace('\\', "/");
        let in_pack = path.strip_prefix(pack).unwrap_or(&path);
        let top = in_pack
            .components()
            .next()
            .map(|component| component.as_os_str().to_string_lossy().into_owned())
            .unwrap_or_default();
        let Some(kind) = file_kind(side, &top) else {
            continue;
        };
        let text = fs::read_to_string(&path).with_context(|| format!("read {file}"))?;
        let value = match json::parse(&text) {
            Ok(value) => value,
            Err(error) => {
                out.broken.push(BrokenFile { file, error });
                continue;
            }
        };
        if kind == "manifest" {
            out.manifests.push(Manifest {
                file,
                min_engine_version: min_engine_version(&value),
            });
            continue;
        }
        let format_version = value
            .get("format_version")
            .map(scalar_text)
            .unwrap_or_default();
        out.files.push(PackFile {
            kind,
            format_version: format_version.clone(),
        });
        let mut extractor = Extractor {
            kind,
            file,
            format_version,
            visited: BTreeSet::new(),
            out: &mut out.occurrences,
        };
        extractor.file(&value);
    }
    Ok(())
}

fn collect_json(dir: &Path, files: &mut Vec<PathBuf>) -> anyhow::Result<()> {
    for entry in fs::read_dir(dir).with_context(|| format!("list {}", dir.display()))? {
        let path = entry?.path();
        if path.is_dir() {
            collect_json(&path, files)?;
        } else if path
            .extension()
            .is_some_and(|extension| extension == "json")
        {
            files.push(path);
        }
    }
    Ok(())
}

fn file_kind(side: Side, top: &str) -> Option<&'static str> {
    Some(match (side, top) {
        (_, "manifest.json") => "manifest",
        (Side::Resource, "ui" | "texts" | "textures" | "sounds" | "sounds.json") => return None,
        (Side::Resource, "entity") => "client_entity",
        (Side::Resource, "attachables") => "attachable",
        (Side::Resource, "animations") => "animation",
        (Side::Resource, "animation_controllers") => "animation_controller",
        (Side::Resource, "render_controllers") => "render_controller",
        (Side::Resource, "particles") => "particle",
        (Side::Resource, "models") => "geometry",
        (Side::Resource, _) => "resource_other",
        (Side::Behavior, "entities") => "bp_entity",
        (Side::Behavior, "items") => "bp_item",
        (Side::Behavior, "blocks") => "bp_block",
        (Side::Behavior, "animations") => "bp_animation",
        (Side::Behavior, "animation_controllers") => "bp_animation_controller",
        (Side::Behavior, _) => "bp_other",
    })
}

fn min_engine_version(manifest: &Json) -> String {
    match manifest
        .get("header")
        .and_then(|header| header.get("min_engine_version"))
    {
        Some(Json::Array(parts)) => parts.iter().map(scalar_text).collect::<Vec<_>>().join("."),
        Some(other) => scalar_text(other),
        None => String::new(),
    }
}

fn scalar_text(value: &Json) -> String {
    match value {
        Json::String(text) | Json::Number(text) => text.clone(),
        Json::Bool(flag) => flag.to_string(),
        _ => String::new(),
    }
}

fn child(pointer: &str, key: &str) -> String {
    let escaped = key.replace('~', "~0").replace('/', "~1");
    format!("{pointer}/{escaped}")
}

fn index(pointer: &str, position: usize) -> String {
    format!("{pointer}/{position}")
}

struct Extractor<'out> {
    kind: &'static str,
    file: String,
    format_version: String,
    visited: BTreeSet<String>,
    out: &'out mut Vec<Occurrence>,
}

impl Extractor<'_> {
    fn file(&mut self, root: &Json) {
        match self.kind {
            "client_entity" => self.entity(root, "minecraft:client_entity"),
            "attachable" => self.entity(root, "minecraft:attachable"),
            "bp_entity" => self.entity(root, "minecraft:entity"),
            "animation" | "bp_animation" => self.animations(root),
            "animation_controller" | "bp_animation_controller" => self.controllers(root),
            "render_controller" => self.render_controllers(root),
            "particle" => self.particle(root),
            "bp_block" => self.block(root),
            _ => {}
        }
        self.sweep(root, "", &[]);
    }

    fn push(&mut self, text: String, form: ValueForm, field: &str, pointer: &str) {
        self.visited.insert(pointer.to_owned());
        self.out.push(Occurrence {
            text,
            form,
            entries: Vec::new(),
            version: None,
            kind: self.kind,
            field: field.to_owned(),
            file: self.file.clone(),
            pointer: pointer.to_owned(),
            format_version: self.format_version.clone(),
        });
    }

    /// Emits one Molang value. An array is a vector of values, each one expression.
    fn emit(&mut self, value: &Json, field: &str, pointer: &str) {
        match value {
            Json::String(text) => self.push(text.clone(), ValueForm::String, field, pointer),
            Json::Number(text) => self.push(text.clone(), ValueForm::Number, field, pointer),
            Json::Bool(flag) => {
                let text = if *flag { "true" } else { "false" };
                self.push(text.to_owned(), ValueForm::Bool, field, pointer);
            }
            Json::Array(items) => {
                for (position, item) in items.iter().enumerate() {
                    self.emit(item, field, &index(pointer, position));
                }
            }
            Json::Object(_) => {
                let Some(Json::String(text)) = value.get("expression") else {
                    return;
                };
                let expression_pointer = child(pointer, "expression");
                self.push(
                    text.clone(),
                    ValueForm::Versioned,
                    field,
                    &expression_pointer,
                );
                if let Some(last) = self.out.last_mut() {
                    last.version = value.get("version").map(scalar_text);
                }
            }
            Json::Null => {}
        }
    }

    /// Emits a script: a string, or an array of strings that the loader joins.
    fn emit_script(&mut self, value: &Json, field: &str, pointer: &str) {
        let Json::Array(items) = value else {
            self.emit(value, field, pointer);
            return;
        };
        let mut entries = Vec::new();
        for (position, item) in items.iter().enumerate() {
            if let Json::String(text) = item {
                entries.push(text.clone());
                self.visited.insert(index(pointer, position));
            }
        }
        self.push(entries.concat(), ValueForm::ScriptArray, field, pointer);
        if let Some(last) = self.out.last_mut() {
            last.entries = entries;
        }
    }

    /// Emits the conditions of a list of `name` or `{name: condition}` entries.
    fn emit_conditions(&mut self, value: &Json, field: &str, pointer: &str) {
        let Json::Array(items) = value else {
            return;
        };
        for (position, item) in items.iter().enumerate() {
            let item_pointer = index(pointer, position);
            for member in item.members() {
                self.emit(&member.value, field, &child(&item_pointer, &member.key));
            }
        }
    }

    fn entity(&mut self, root: &Json, key: &str) {
        let Some(description) = root.get(key).and_then(|entity| entity.get("description")) else {
            return;
        };
        let pointer = child(&child("", key), "description");
        if let Some(scripts) = description.get("scripts") {
            let scripts_pointer = child(&pointer, "scripts");
            for member in scripts.members() {
                let member_pointer = child(&scripts_pointer, &member.key);
                let field = format!("scripts.{}", member.key);
                match member.key.as_str() {
                    "initialize" | "pre_animation" => {
                        self.emit_script(&member.value, &field, &member_pointer);
                    }
                    "animate" => self.emit_conditions(&member.value, &field, &member_pointer),
                    "variables" => {}
                    _ => self.emit(&member.value, &field, &member_pointer),
                }
            }
        }
        if let Some(items) = description.get("item") {
            let items_pointer = child(&pointer, "item");
            for item in items.members() {
                self.emit(
                    &item.value,
                    "item.condition",
                    &child(&items_pointer, &item.key),
                );
            }
        }
        if let Some(controllers) = description.get("render_controllers") {
            let field = "render_controllers.condition";
            self.emit_conditions(controllers, field, &child(&pointer, "render_controllers"));
        }
    }

    fn animations(&mut self, root: &Json) {
        let Some(animations) = root.get("animations") else {
            return;
        };
        for animation in animations.members() {
            let pointer = child(&child("", "animations"), &animation.key);
            for member in animation.value.members() {
                let member_pointer = child(&pointer, &member.key);
                match member.key.as_str() {
                    "anim_time_update" | "blend_weight" | "start_delay" | "loop_delay" => {
                        self.emit(&member.value, &member.key, &member_pointer);
                    }
                    "loop" => {
                        if let Json::String(text) = &member.value
                            && text != "hold_on_last_frame"
                        {
                            self.emit(&member.value, "loop", &member_pointer);
                        }
                    }
                    "bones" => self.bones(&member.value, &member_pointer),
                    "timeline" => {
                        for keyframe in member.value.members() {
                            let keyframe_pointer = child(&member_pointer, &keyframe.key);
                            self.emit_script(&keyframe.value, "timeline", &keyframe_pointer);
                        }
                    }
                    "particle_effects" => {
                        for keyframe in member.value.members() {
                            let keyframe_pointer = child(&member_pointer, &keyframe.key);
                            self.pre_effect_scripts(&keyframe.value, &keyframe_pointer);
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    fn pre_effect_scripts(&mut self, value: &Json, pointer: &str) {
        let field = "particle_effects.pre_effect_script";
        match value {
            Json::Array(items) => {
                for (position, item) in items.iter().enumerate() {
                    self.pre_effect_scripts(item, &index(pointer, position));
                }
            }
            Json::Object(_) => {
                if let Some(script) = value.get("pre_effect_script") {
                    self.emit_script(script, field, &child(pointer, "pre_effect_script"));
                }
            }
            _ => {}
        }
    }

    fn bones(&mut self, bones: &Json, pointer: &str) {
        for bone in bones.members() {
            let bone_pointer = child(pointer, &bone.key);
            for channel in bone.value.members() {
                if !matches!(channel.key.as_str(), "rotation" | "position" | "scale") {
                    continue;
                }
                let channel_pointer = child(&bone_pointer, &channel.key);
                let field = format!("bones.{}", channel.key);
                if !matches!(channel.value, Json::Object(_)) {
                    self.emit(&channel.value, &field, &channel_pointer);
                    continue;
                }
                let keyframe_field = format!("{field}.keyframe");
                for keyframe in channel.value.members() {
                    let keyframe_pointer = child(&channel_pointer, &keyframe.key);
                    self.keyframe(&keyframe.value, &keyframe_field, &keyframe_pointer);
                }
            }
        }
    }

    fn keyframe(&mut self, value: &Json, field: &str, pointer: &str) {
        if !matches!(value, Json::Object(_)) || value.get("expression").is_some() {
            self.emit(value, field, pointer);
            return;
        }
        for key in ["pre", "post"] {
            if let Some(side) = value.get(key) {
                self.emit(side, field, &child(pointer, key));
            }
        }
    }

    fn controllers(&mut self, root: &Json) {
        let Some(controllers) = root.get("animation_controllers") else {
            return;
        };
        for controller in controllers.members() {
            let pointer = child(&child("", "animation_controllers"), &controller.key);
            let Some(states) = controller.value.get("states") else {
                continue;
            };
            let states_pointer = child(&pointer, "states");
            for state in states.members() {
                self.state(&state.value, &child(&states_pointer, &state.key));
            }
        }
    }

    fn state(&mut self, state: &Json, pointer: &str) {
        for member in state.members() {
            let member_pointer = child(pointer, &member.key);
            let field = format!("states.{}", member.key);
            match member.key.as_str() {
                "animations" | "transitions" => {
                    self.emit_conditions(&member.value, &field, &member_pointer);
                }
                "on_entry" | "on_exit" => self.emit_script(&member.value, &field, &member_pointer),
                "parameters" => self.emit(&member.value, &field, &member_pointer),
                "variables" => {
                    for variable in member.value.members() {
                        let Some(input) = variable.value.get("input") else {
                            continue;
                        };
                        let variable_pointer = child(&member_pointer, &variable.key);
                        let input_pointer = child(&variable_pointer, "input");
                        self.emit(input, "states.variables.input", &input_pointer);
                    }
                }
                "particle_effects" => self.pre_effect_scripts(&member.value, &member_pointer),
                _ => {}
            }
        }
    }

    fn render_controllers(&mut self, root: &Json) {
        let Some(controllers) = root.get("render_controllers") else {
            return;
        };
        for controller in controllers.members() {
            let pointer = child(&child("", "render_controllers"), &controller.key);
            for member in controller.value.members() {
                let member_pointer = child(&pointer, &member.key);
                self.render_member(&member.key, &member.value, &member_pointer);
            }
        }
    }

    fn render_member(&mut self, key: &str, value: &Json, pointer: &str) {
        match key {
            "arrays" => {
                for kind in value.members() {
                    let kind_pointer = child(pointer, &kind.key);
                    let field = format!("arrays.{}", kind.key);
                    for array in kind.value.members() {
                        self.emit(&array.value, &field, &child(&kind_pointer, &array.key));
                    }
                }
            }
            "materials" | "part_visibility" => self.emit_conditions(value, key, pointer),
            "color" | "overlay_color" | "on_fire_color" | "is_hurt_color" | "uv_anim" => {
                for channel in value.members() {
                    let field = format!("{key}.{}", channel.key);
                    self.emit(&channel.value, &field, &child(pointer, &channel.key));
                }
            }
            _ => self.emit(value, key, pointer),
        }
    }

    fn particle(&mut self, root: &Json) {
        let Some(effect) = root.get("particle_effect") else {
            return;
        };
        let pointer = child("", "particle_effect");
        if let Some(components) = effect.get("components") {
            let components_pointer = child(&pointer, "components");
            for component in components.members() {
                let component_pointer = child(&components_pointer, &component.key);
                let field = format!("components.{}", component.key);
                self.particle_value(&component.value, &field, &component_pointer);
            }
        }
        if let Some(curves) = effect.get("curves") {
            let curves_pointer = child(&pointer, "curves");
            for curve in curves.members() {
                let curve_pointer = child(&curves_pointer, &curve.key);
                for key in ["input", "horizontal_range", "nodes"] {
                    if let Some(value) = curve.value.get(key) {
                        self.emit(value, &format!("curves.{key}"), &child(&curve_pointer, key));
                    }
                }
            }
        }
        if let Some(events) = effect.get("events") {
            self.particle_events(events, &child(&pointer, "events"));
        }
    }

    fn particle_events(&mut self, value: &Json, pointer: &str) {
        match value {
            Json::Object(members) => {
                for member in members {
                    let member_pointer = child(pointer, &member.key);
                    if member.key == "expression" {
                        self.emit(&member.value, "events.expression", &member_pointer);
                    } else {
                        self.particle_events(&member.value, &member_pointer);
                    }
                }
            }
            Json::Array(items) => {
                for (position, item) in items.iter().enumerate() {
                    self.particle_events(item, &index(pointer, position));
                }
            }
            _ => {}
        }
    }

    /// Emits each string of a particle component, except the keys and the values that are
    /// names, modes or colours.
    fn particle_value(&mut self, value: &Json, field: &str, pointer: &str) {
        const NOT_MOLANG: [&str; 9] = [
            "facing_camera_mode",
            "mode",
            "texture",
            "material",
            "event",
            "events",
            "timeline",
            "creation_event",
            "expiration_event",
        ];
        match value {
            Json::String(text) => {
                let is_resource_name =
                    text.contains(':') && !text.contains([' ', '?']) && !looks_like_molang(text);
                let is_name = text.starts_with('#')
                    || is_resource_name
                    || matches!(text.as_str(), "inwards" | "outwards");
                if !is_name {
                    self.emit(value, field, pointer);
                }
            }
            Json::Array(items) => {
                for (position, item) in items.iter().enumerate() {
                    self.particle_value(item, field, &index(pointer, position));
                }
            }
            Json::Object(members) => {
                if value.get("expression").is_some() {
                    self.emit(value, field, pointer);
                    return;
                }
                for member in members {
                    if NOT_MOLANG.contains(&member.key.as_str()) {
                        continue;
                    }
                    let is_gradient_stop = member.key.parse::<f64>().is_ok();
                    let member_field = if is_gradient_stop {
                        field.to_owned()
                    } else {
                        format!("{field}.{}", member.key)
                    };
                    self.particle_value(&member.value, &member_field, &child(pointer, &member.key));
                }
            }
            _ => {}
        }
    }

    fn block(&mut self, root: &Json) {
        let Some(block) = root.get("minecraft:block") else {
            return;
        };
        let pointer = child("", "minecraft:block");
        if let Some(Json::Array(permutations)) = block.get("permutations") {
            let permutations_pointer = child(&pointer, "permutations");
            for (position, permutation) in permutations.iter().enumerate() {
                if let Some(condition) = permutation.get("condition") {
                    let condition_pointer =
                        child(&index(&permutations_pointer, position), "condition");
                    self.emit(condition, "permutations.condition", &condition_pointer);
                }
            }
        }
    }

    /// Emits each other string that looks like Molang, with the last two keys as the field.
    fn sweep(&mut self, value: &Json, pointer: &str, keys: &[&str]) {
        match value {
            Json::String(text) if !self.visited.contains(pointer) && looks_like_molang(text) => {
                let tail = keys.get(keys.len().saturating_sub(2)..).unwrap_or_default();
                let field = format!("other:{}", tail.join("."));
                self.push(text.clone(), ValueForm::String, &field, pointer);
            }
            Json::Array(items) => {
                for (position, item) in items.iter().enumerate() {
                    self.sweep(item, &index(pointer, position), keys);
                }
            }
            Json::Object(members) => {
                for member in members {
                    let mut inner = keys.to_vec();
                    inner.push(&member.key);
                    self.sweep(&member.value, &child(pointer, &member.key), &inner);
                }
            }
            _ => {}
        }
    }
}

/// A string looks like Molang when it has a namespace followed by `.` and a name.
fn looks_like_molang(text: &str) -> bool {
    const NAMESPACES: [&str; 9] = [
        "q", "query", "v", "variable", "t", "temp", "c", "context", "math",
    ];
    let lower = text.to_ascii_lowercase();
    let bytes = lower.as_bytes();
    let mut start = 0;
    while start < bytes.len() {
        let word_end = bytes
            .get(start..)
            .and_then(|rest| {
                rest.iter()
                    .position(|byte| !(byte.is_ascii_alphanumeric() || *byte == b'_'))
            })
            .map_or(bytes.len(), |length| start + length);
        let word = lower.get(start..word_end).unwrap_or_default();
        let before = start
            .checked_sub(1)
            .and_then(|position| bytes.get(position));
        let clean_before = !matches!(before, Some(b'/' | b'.' | b':'));
        let followed = bytes.get(word_end) == Some(&b'.')
            && bytes
                .get(word_end + 1)
                .is_some_and(|byte| byte.is_ascii_alphabetic() || *byte == b'_');
        if clean_before && followed && NAMESPACES.contains(&word) {
            return true;
        }
        start = word_end.max(start) + 1;
    }
    false
}
