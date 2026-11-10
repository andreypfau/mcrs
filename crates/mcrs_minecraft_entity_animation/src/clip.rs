use mcrs_minecraft_pack_file::{DiagnosticKind, FieldCompiler, Json, JsonObject};
use mcrs_molang::Expression;

use crate::channel::Channel;

/// What a clip does when its time passes its length.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum LoopMode {
    /// The clip finishes and adds nothing more.
    #[default]
    Once,
    Loop,
    HoldOnLastFrame,
}

/// How the time of an instance changes in a frame.
#[derive(Clone, Debug, PartialEq)]
pub enum TimeUpdate {
    /// `query.anim_time + query.delta_time`: the time is the change of the entity clock since
    /// the start of the instance, in seconds.
    Clock,
    Expression(Expression),
}

/// The channels of one bone in one clip.
#[derive(Clone, Debug, PartialEq)]
pub struct ClipBone {
    /// The index of the bone in [`crate::Rig::bones`].
    pub bone: usize,
    pub rotation: Option<Channel>,
    pub position: Option<Channel>,
    pub scale: Option<Channel>,
    /// `relative_to.rotation` is `"entity"`.
    pub relative_to_entity: bool,
}

/// A compiled clip of an animation file.
#[derive(Clone, Debug, PartialEq)]
pub struct Clip {
    pub identifier: String,
    pub loop_mode: LoopMode,
    /// The length in seconds; `0` for a clip that never finishes.
    pub length: f32,
    pub time_update: TimeUpdate,
    pub blend_weight: Expression,
    pub override_previous: bool,
    pub start_delay: Expression,
    pub loop_delay: Expression,
    pub bones: Vec<ClipBone>,
}

impl Clip {
    /// Compiles a clip. `bone` gives the index of a bone name in lower case.
    pub(crate) fn compile(
        identifier: &str,
        object: &JsonObject,
        compiler: &mut FieldCompiler,
        mut bone: impl FnMut(&str) -> usize,
    ) -> Self {
        let field = |name: &str, default: f32, compiler: &mut FieldCompiler| match object.get(name)
        {
            Some(json) => compiler.compile(json, &format!("{identifier}/{name}")),
            None => FieldCompiler::constant(default),
        };
        let blend_weight = field("blend_weight", 1.0, compiler);
        let start_delay = field("start_delay", 0.0, compiler);
        let loop_delay = field("loop_delay", 0.0, compiler);
        let time_update = match object.get("anim_time_update") {
            Some(json) => TimeUpdate::Expression(
                compiler.compile(json, &format!("{identifier}/anim_time_update")),
            ),
            None => TimeUpdate::Clock,
        };
        let loop_mode = match object.get("loop") {
            None | Some(Json::Bool(false)) => LoopMode::Once,
            Some(Json::Bool(true)) => LoopMode::Loop,
            Some(Json::String(mode)) if mode == "hold_on_last_frame" => LoopMode::HoldOnLastFrame,
            Some(_) => {
                content_error(
                    compiler,
                    identifier,
                    "`loop` is not a bool or `hold_on_last_frame`",
                );
                LoopMode::Once
            }
        };
        let override_previous = match object.get("override_previous_animation") {
            None => false,
            Some(Json::Bool(value)) => *value,
            Some(_) => {
                content_error(
                    compiler,
                    identifier,
                    "`override_previous_animation` is not a bool",
                );
                false
            }
        };
        let bones = bones(identifier, object, compiler, &mut bone);
        let length = match object.get("animation_length") {
            Some(Json::Number(length)) if *length >= 0.0 => *length as f32,
            other => {
                if other.is_some() {
                    content_error(
                        compiler,
                        identifier,
                        "`animation_length` is not a number ≥ 0",
                    );
                }
                bones
                    .iter()
                    .flat_map(|bone| [&bone.rotation, &bone.position, &bone.scale])
                    .flatten()
                    .map(Channel::end)
                    .fold(0.0, f32::max)
            }
        };
        for effect in ["sound_effects", "particle_effects", "timeline"] {
            if object
                .get(effect)
                .is_some_and(|json| json.as_object().is_none())
            {
                content_error(
                    compiler,
                    identifier,
                    &format!("`{effect}` is not an object"),
                );
            }
        }
        Self {
            identifier: identifier.into(),
            loop_mode,
            length,
            time_update,
            blend_weight,
            override_previous,
            start_delay,
            loop_delay,
            bones,
        }
    }
}

fn bones(
    identifier: &str,
    clip: &JsonObject,
    compiler: &mut FieldCompiler,
    bone: &mut impl FnMut(&str) -> usize,
) -> Vec<ClipBone> {
    let Some(object) = clip.get("bones").and_then(Json::as_object) else {
        if clip.get("bones").is_some() {
            content_error(compiler, identifier, "`bones` is not an object");
        }
        return Vec::new();
    };
    let mut bones: Vec<ClipBone> = Vec::with_capacity(object.len());
    for (name, value) in object.iter() {
        let context = format!("{identifier}/bones/{name}");
        let Some(channels) = value.as_object() else {
            content_error(compiler, &context, "a bone is not an object");
            continue;
        };
        let channel = |kind: &str, compiler: &mut FieldCompiler| {
            channels
                .get(kind)
                .and_then(|json| Channel::compile(json, compiler, &format!("{context}/{kind}")))
        };
        let rotation = channel("rotation", compiler);
        let position = channel("position", compiler);
        let scale = channel("scale", compiler);
        let relative_to_entity = match channels.get("relative_to").and_then(Json::as_object) {
            Some(relative) => match relative.get("rotation") {
                Some(Json::String(target)) if target == "entity" => true,
                _ => {
                    compiler.report(
                        DiagnosticKind::Unsupported,
                        &context,
                        "`relative_to` other than `rotation: entity`",
                    );
                    false
                }
            },
            None => false,
        };
        let index = bone(&name.to_ascii_lowercase());
        let clip_bone = ClipBone {
            bone: index,
            rotation,
            position,
            scale,
            relative_to_entity,
        };
        match bones.iter_mut().find(|held| held.bone == index) {
            Some(held) => *held = clip_bone,
            None => bones.push(clip_bone),
        }
    }
    bones
}

fn content_error(compiler: &mut FieldCompiler, context: &str, message: &str) {
    compiler.report(DiagnosticKind::ContentError, context, message);
}
