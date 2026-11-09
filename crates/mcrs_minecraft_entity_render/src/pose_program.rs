use std::fmt::Write;

use glam::Vec3;
use mcrs_minecraft_entity_animation::{
    AnimateTarget, Channel, Clip, EntityQuery, LerpMode, LoopMode, Rig, TimeUpdate,
};
use mcrs_molang::{ChannelClass, Expression, InputRange, PureInput, QuerySlot, Value};

use crate::records::{CLIP_SLOTS, INPUT_VALUES};

/// Where the pose pass of a definition takes the weight and the time of an `animate` entry.
#[derive(Clone, Debug, PartialEq)]
pub struct EntryInputs {
    /// The input value that holds the weight, which the CPU computes in each frame; `None` when
    /// the GPU computes it.
    pub weight: Option<usize>,
    /// The clip slot that holds the start of a clip with the time of the entity clock.
    pub clip: Option<usize>,
    /// The input value that holds the time, for a time update that the GPU cannot compute.
    pub time: Option<usize>,
}

/// The inputs that the CPU writes for a definition: one entry for each `animate` entry.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct InputLayout {
    pub entries: Vec<EntryInputs>,
}

impl InputLayout {
    /// Whether a slot of this definition has an input record.
    #[must_use]
    pub fn is_used(&self) -> bool {
        self.entries
            .iter()
            .any(|entry| entry.weight.is_some() || entry.clip.is_some() || entry.time.is_some())
    }
}

/// The WGSL of the animated offsets of one client entity definition, and the inputs it reads.
#[derive(Clone, Debug, PartialEq)]
pub struct PoseProgram {
    /// The functions of the channels, then `fn pose_offsets(ctx: PoseContext)`.
    pub wgsl: String,
    pub inputs: InputLayout,
}

/// Why a definition has no GPU pose.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PoseProgramError(pub String);

impl std::fmt::Display for PoseProgramError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for PoseProgramError {}

/// The WGSL value of a pose input that the pose pass computes from the slot records, with its
/// range; `None` for a query that the pose pass does not compute.
fn pose_input(query: EntityQuery) -> Option<(&'static str, InputRange)> {
    use EntityQuery as Q;
    let angle = InputRange::new(-180.0, 180.0);
    Some(match query {
        Q::ModifiedDistanceMoved => ("ctx.walk_position", InputRange::FINITE),
        Q::ModifiedMoveSpeed => ("ctx.walk_speed", InputRange::new(0.0, 1.0)),
        Q::TargetXRotation | Q::HeadXRotation(Some(0)) => {
            ("ctx.pitch", InputRange::new(-180.0, 180.0))
        }
        Q::TargetYRotation => ("ctx.relative_head_yaw", angle),
        Q::BodyYRotation => ("ctx.body_yaw", angle),
        Q::HeadYRotation(Some(0)) => ("ctx.head_yaw", angle),
        Q::LifeTime => ("ctx.life_time", InputRange::new(0.0, f32::MAX)),
        Q::FrameAlpha => ("ctx.partial_tick", InputRange::new(0.0, 1.0)),
        Q::DeathTicks => ("ctx.death_time", InputRange::new(0.0, f32::MAX)),
        Q::IsBaby => ("ctx.baby", InputRange::BOOLEAN),
        Q::IsAlive => ("ctx.alive", InputRange::BOOLEAN),
        _ => return None,
    })
}

struct Generator<'a> {
    rig: &'a Rig,
    /// The palette index of each rig bone.
    palette_of_bone: &'a [usize],
    /// The rest rotation in file space of each palette entry.
    rest: &'a [Vec3],
    functions: String,
    body: String,
    next_function: usize,
}

impl PoseProgram {
    /// Generates the pose program of `rig`. `palette_of_bone` gives the palette index of each
    /// rig bone and `rest` the rest rotation in file space of each palette entry, which `this`
    /// reads.
    pub fn generate(
        rig: &Rig,
        palette_of_bone: &[usize],
        rest: &[Vec3],
    ) -> Result<Self, PoseProgramError> {
        let inputs = input_layout(rig)?;
        let mut generator = Generator {
            rig,
            palette_of_bone,
            rest,
            functions: String::new(),
            body: String::new(),
            next_function: 0,
        };
        for (index, entry) in rig.animate.iter().enumerate() {
            let AnimateTarget::Clip(clip) = entry.target else {
                continue;
            };
            let Some(clip) = rig.clips.get(clip) else {
                continue;
            };
            let Some(layout) = inputs.entries.get(index) else {
                continue;
            };
            generator.entry(&entry.weight, clip, layout)?;
        }
        let mut wgsl = generator.functions;
        wgsl.push_str("fn pose_offsets(ctx: PoseContext) {\n");
        wgsl.push_str(&generator.body);
        wgsl.push_str("}\n");
        Ok(Self { wgsl, inputs })
    }
}

/// A definition with a weight that is not a constant reads its weights from the CPU; then each
/// clip that plays on the entity clock needs the start that the CPU gives.
fn input_layout(rig: &Rig) -> Result<InputLayout, PoseProgramError> {
    let constant = |expression: &Expression| expression.as_constant().is_some();
    let needs_input = rig.animate.iter().any(|entry| {
        let AnimateTarget::Clip(clip) = entry.target else {
            return true;
        };
        !constant(&entry.weight)
            || rig
                .clips
                .get(clip)
                .is_some_and(|clip| !constant(&clip.blend_weight))
    });
    let mut layout = InputLayout::default();
    let (mut values, mut clips) = (0usize, 0usize);
    for entry in &rig.animate {
        let clip = match entry.target {
            AnimateTarget::Clip(clip) => rig.clips.get(clip),
            _ => None,
        };
        let Some(clip) = clip else {
            layout.entries.push(EntryInputs {
                weight: None,
                clip: None,
                time: None,
            });
            continue;
        };
        for delay in [&clip.start_delay, &clip.loop_delay] {
            if delay
                .as_constant()
                .is_none_or(|value| value.as_number() != 0.0)
            {
                return Err(PoseProgramError(format!(
                    "{}: a start or loop delay is not played on the GPU yet",
                    clip.identifier
                )));
            }
        }
        let mut inputs = EntryInputs {
            weight: None,
            clip: None,
            time: None,
        };
        if needs_input {
            inputs.weight = Some(values);
            values += 1;
            match &clip.time_update {
                TimeUpdate::Clock => {
                    inputs.clip = Some(clips);
                    clips += 1;
                }
                TimeUpdate::Expression(expression) => {
                    if !matches!(
                        classify(rig, expression),
                        ChannelClass::Pure(_) | ChannelClass::Constant
                    ) {
                        inputs.time = Some(values);
                        values += 1;
                    }
                }
            }
        }
        layout.entries.push(inputs);
    }
    if values > INPUT_VALUES || clips > CLIP_SLOTS {
        return Err(PoseProgramError(format!(
            "{}: {values} input values and {clips} clip slots are more than an input record holds",
            rig.identifier
        )));
    }
    Ok(layout)
}

fn classify(rig: &Rig, expression: &Expression) -> ChannelClass {
    expression.classify(|slot| {
        let query = rig.slot_query(slot)?;
        if query == EntityQuery::AnimTime {
            return Some(InputRange::new(0.0, f32::MAX));
        }
        pose_input(query).map(|(_, range)| range)
    })
}

fn literal(value: f32) -> String {
    format!("bitcast<f32>({:#010x}u)", value.to_bits())
}

impl Generator<'_> {
    fn entry(
        &mut self,
        weight: &Expression,
        clip: &Clip,
        layout: &EntryInputs,
    ) -> Result<(), PoseProgramError> {
        let body = &mut String::new();
        let weight = match layout.weight {
            Some(value) => format!("input_value(ctx, {value}u)"),
            None => {
                let entry = self.scalar(weight, "0.0", "0.0", &clip.identifier)?;
                let blend = self.scalar(&clip.blend_weight, "0.0", "0.0", &clip.identifier)?;
                format!("({entry}) * ({blend})")
            }
        };
        let _ = writeln!(body, "    // {}", clip.identifier);
        let _ = writeln!(body, "    {{\n    let w = {weight};");
        body.push_str("    if (w != 0.0 && w == w) {\n");
        let time = match (&clip.time_update, layout.clip, layout.time) {
            (_, _, Some(value)) => format!("input_value(ctx, {value}u)"),
            (TimeUpdate::Clock, Some(slot), None) => {
                format!("(ctx.clock - clip_start(ctx, {slot}u)) / 20.0")
            }
            (TimeUpdate::Clock, None, None) => "ctx.clock / 20.0".to_owned(),
            (TimeUpdate::Expression(expression), None, None) => {
                self.scalar(expression, "0.0", "0.0", &clip.identifier)?
            }
            (TimeUpdate::Expression(_), Some(_), None) => "0.0".to_owned(),
        };
        let _ = writeln!(body, "    var t = {time};");
        let length = clip.length;
        if length > 0.0 {
            match clip.loop_mode {
                LoopMode::Loop => {
                    let _ = writeln!(
                        body,
                        "    t = t - floor(t / {l}) * {l};\n    t = select(t, 0.0, t >= {l});",
                        l = literal(length)
                    );
                }
                LoopMode::HoldOnLastFrame => {
                    let _ = writeln!(body, "    t = min(t, {});", literal(length));
                }
                LoopMode::Once => {
                    let _ = writeln!(body, "    if (t < {}) {{", literal(length));
                }
            }
        }
        for clip_bone in &clip.bones {
            let Some(&palette) = self.palette_of_bone.get(clip_bone.bone) else {
                continue;
            };
            let rest = self.rest.get(palette).copied().unwrap_or(Vec3::ZERO);
            if clip.override_previous {
                let _ = writeln!(
                    body,
                    "    rotation[{palette}] = vec3<f32>(0.0); position[{palette}] = vec3<f32>(0.0);"
                );
                if clip_bone.scale.is_some() {
                    let _ = writeln!(body, "    scale[{palette}] = vec3<f32>(1.0);");
                }
            }
            for (kind, channel) in [
                ("rotation", &clip_bone.rotation),
                ("position", &clip_bone.position),
                ("scale", &clip_bone.scale),
            ] {
                let Some(channel) = channel else {
                    continue;
                };
                let this = |axis: usize| match kind {
                    "rotation" => format!(
                        "({} + rotation[{palette}][{axis}])",
                        literal(rest.to_array().get(axis).copied().unwrap_or(0.0))
                    ),
                    _ => format!("{kind}[{palette}][{axis}]"),
                };
                let context = format!("{}/{}", clip.identifier, kind);
                let sample = self.sample(channel, &this, &context)?;
                let _ = writeln!(body, "    {{\n{sample}");
                let apply = match kind {
                    "scale" => format!(
                        "        scale[{palette}] = scale[{palette}] * (1.0 + (v - 1.0) * w);"
                    ),
                    _ => format!("        {kind}[{palette}] = {kind}[{palette}] + w * v;"),
                };
                let _ = writeln!(body, "{apply}\n    }}");
            }
        }
        if length > 0.0 && clip.loop_mode == LoopMode::Once {
            body.push_str("    }\n");
        }
        body.push_str("    }\n    }\n");
        self.body.push_str(body);
        Ok(())
    }

    /// WGSL that declares `v`, the sample of `channel` at the time `t`, limited to the pose range.
    fn sample(
        &mut self,
        channel: &Channel,
        this: &impl Fn(usize) -> String,
        context: &str,
    ) -> Result<String, PoseProgramError> {
        let keys = &channel.keyframes;
        let mut out = String::from("        var v = vec3<f32>(0.0);\n");
        let Some(last) = keys.len().checked_sub(1) else {
            return Ok(out);
        };
        let side = |generator: &mut Self, values: &[Expression; 3]| {
            let mut axes = Vec::with_capacity(3);
            for (axis, expression) in values.iter().enumerate() {
                axes.push(generator.scalar(expression, &this(axis), "t", context)?);
            }
            Ok::<String, PoseProgramError>(format!("vec3<f32>({})", axes.join(", ")))
        };
        let (Some(first), Some(last)) = (keys.first(), keys.get(last)) else {
            return Ok(out);
        };
        let pre = side(self, &first.pre)?;
        let _ = writeln!(
            out,
            "        if (t < {}) {{ v = {pre}; }}",
            literal(first.time)
        );
        for (from, pair) in keys.windows(2).enumerate() {
            let [a, b] = pair else {
                continue;
            };
            let p1 = side(self, &a.post)?;
            let p2 = side(self, &b.pre)?;
            let _ = writeln!(
                out,
                "        else if (t < {tb}) {{\n            let alpha = (t - {ta}) / ({tb} - {ta});\n            let p1 = {p1};\n            let p2 = {p2};",
                ta = literal(a.time),
                tb = literal(b.time),
            );
            if a.lerp_mode == LerpMode::CatmullRom || b.lerp_mode == LerpMode::CatmullRom {
                let p0 = match from.checked_sub(1).and_then(|index| keys.get(index)) {
                    Some(before) => side(self, &before.post)?,
                    None => "p1".to_owned(),
                };
                let p3 = match keys.get(from + 2) {
                    Some(after) => side(self, &after.pre)?,
                    None => "p2".to_owned(),
                };
                let _ = writeln!(
                    out,
                    "            v = catmull_rom({p0}, p1, p2, {p3}, alpha);\n        }}"
                );
            } else {
                out.push_str("            v = p1 + (p2 - p1) * alpha;\n        }\n");
            }
        }
        let post = side(self, &last.post)?;
        let _ = writeln!(out, "        else {{ v = {post}; }}");
        out.push_str("        v = clamp(v, vec3<f32>(-1e6), vec3<f32>(1e6));\n");
        Ok(out)
    }

    /// A WGSL expression of one number field. `this` is the value of `this`, and `time` the
    /// WGSL value of `query.anim_time`.
    fn scalar(
        &mut self,
        expression: &Expression,
        this: &str,
        time: &str,
        context: &str,
    ) -> Result<String, PoseProgramError> {
        match expression {
            Expression::Constant(Value::Number(value)) => Ok(literal(*value)),
            Expression::Constant(_) => Ok(literal(0.0)),
            Expression::Program(_) => match classify(self.rig, expression) {
                ChannelClass::Constant => Ok(literal(0.0)),
                ChannelClass::Pure(program) => {
                    let name = format!("channel_{}", self.next_function);
                    self.next_function += 1;
                    self.functions
                        .push_str(&mcrs_molang_wgsl::lower(&program, &name));
                    let mut call = format!("{name}(frame.opaque_zero");
                    for input in program.inputs() {
                        let argument = match input {
                            PureInput::This => this.to_owned(),
                            PureInput::Query(slot) => self.query(*slot, time, context)?,
                            _ => {
                                return Err(PoseProgramError(format!(
                                    "{context}: a channel reads a variable, which the GPU pose cannot give yet"
                                )));
                            }
                        };
                        let _ = write!(call, ", {argument}");
                    }
                    call.push(')');
                    Ok(call)
                }
                ChannelClass::State(reason) => Err(PoseProgramError(format!(
                    "{context}: a channel runs on the CPU ({reason:?}), which the GPU pose cannot read yet"
                ))),
                other => Err(PoseProgramError(format!(
                    "{context}: a channel has the class {other:?}, which the GPU pose cannot read yet"
                ))),
            },
        }
    }

    fn query(
        &self,
        slot: QuerySlot,
        time: &str,
        context: &str,
    ) -> Result<String, PoseProgramError> {
        let query = self.rig.slot_query(slot);
        if query == Some(EntityQuery::AnimTime) {
            return Ok(time.to_owned());
        }
        query
            .and_then(pose_input)
            .map(|(wgsl, _)| wgsl.to_owned())
            .ok_or_else(|| PoseProgramError(format!("{context}: a query that is not a pose input")))
    }
}
