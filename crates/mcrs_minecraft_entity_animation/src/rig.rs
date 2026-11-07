use mcrs_molang::{Accesses, Expression, QueryRegistry, QuerySlot, VariableLayout, VariableSlot};

use crate::animation_diagnostic::{AnimationDiagnostic, DiagnosticKind};
use crate::client_entity_definition::ClientEntityDefinition;
use crate::clip::Clip;
use crate::clip_library::ClipLibrary;
use crate::entity_query::EntityQuery;
use crate::field_compiler::FieldCompiler;

/// What an element of the `animate` list plays.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum AnimateTarget {
    /// The index of the clip in [`Rig::clips`].
    Clip(usize),
}

/// An element of the `animate` list with its compiled weight.
#[derive(Clone, Debug, PartialEq)]
pub struct AnimateEntry {
    pub short_name: String,
    pub weight: Expression,
    pub target: AnimateTarget,
}

/// A query slot that the query source fills.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct HostQuery {
    pub slot: QuerySlot,
    pub query: EntityQuery,
}

/// The query slots of a rig.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct QueryBindings {
    pub host: Vec<HostQuery>,
    pub delta_time: Option<QuerySlot>,
    pub anim_time: Option<QuerySlot>,
    pub key_frame_lerp_time: Option<QuerySlot>,
    pub count: usize,
}

/// When the client runs a script.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Evaluation {
    EveryFrame,
    /// Only when one of these inputs changed since the last run; otherwise the run would give
    /// the same result and the same variables.
    OnChange {
        queries: Vec<QuerySlot>,
        variables: Vec<VariableSlot>,
    },
}

/// A script of the client entity and the frames in which it runs.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Script {
    pub expressions: Vec<Expression>,
    pub evaluation: Evaluation,
}

/// The animation of one client entity, compiled: its clips, its play list and its scripts, with
/// one variable layout and one set of query slots for every expression.
#[derive(Clone, Debug, PartialEq)]
pub struct Rig {
    pub identifier: String,
    /// The bone names that the clips animate, in lower case.
    pub bones: Vec<String>,
    pub clips: Vec<Clip>,
    pub animate: Vec<AnimateEntry>,
    pub(crate) initialize: Option<Expression>,
    pub(crate) pre_animation: Option<Script>,
    /// `scale`, `scalex`, `scaley` and `scalez`.
    pub(crate) scale: Script,
    pub(crate) layout: VariableLayout,
    pub(crate) queries: QueryBindings,
}

impl Rig {
    /// Compiles the client entity with the clips of `library`. A problem gives a diagnostic and
    /// the default of the field.
    pub fn compile(
        definition: &ClientEntityDefinition,
        library: &ClipLibrary,
        diagnostics: &mut Vec<AnimationDiagnostic>,
    ) -> Self {
        let mut compiler = FieldCompiler::new();
        let identifier = definition.identifier.as_str();
        let scripts = &definition.scripts;
        let initialize = (!scripts.initialize.is_empty()).then(|| {
            let entries: Vec<&str> = scripts.initialize.iter().map(String::as_str).collect();
            let context = format!("{identifier}/scripts/initialize");
            compiler.compile_script(&entries, &context)
        });
        let pre_animation = (!scripts.pre_animation.is_empty()).then(|| {
            let entries: Vec<&str> = scripts.pre_animation.iter().map(String::as_str).collect();
            let context = format!("{identifier}/scripts/pre_animation");
            compiler.compile_script(&entries, &context)
        });
        let scale: Vec<Expression> = scripts
            .scale
            .iter()
            .zip(["scale", "scalex", "scaley", "scalez"])
            .map(|(json, name)| match json {
                Some(json) => compiler.compile(json, &format!("{identifier}/scripts/{name}")),
                None => FieldCompiler::constant(1.0),
            })
            .collect();
        let mut bones: Vec<String> = Vec::new();
        let mut clips: Vec<Clip> = Vec::new();
        let mut animate = Vec::with_capacity(scripts.animate.len());
        for item in &scripts.animate {
            let context = format!("{identifier}/scripts/animate/{}", item.short_name);
            let Some(target) = resolve(
                definition,
                library,
                &item.short_name,
                Resolved {
                    clips: &mut clips,
                    bones: &mut bones,
                },
                &mut compiler,
                &context,
            ) else {
                continue;
            };
            let weight = match &item.weight {
                Some(json) => compiler.compile(json, &context),
                None => FieldCompiler::constant(1.0),
            };
            animate.push(AnimateEntry {
                short_name: item.short_name.clone(),
                weight,
                target,
            });
        }
        let queries = bindings(&compiler.registry);
        let pre_animation =
            pre_animation.map(|expression| Script::new(&compiler, vec![expression]));
        let scale = Script::new(&compiler, scale);
        diagnostics.append(&mut compiler.diagnostics);
        Self {
            identifier: identifier.into(),
            bones,
            clips,
            animate,
            initialize,
            pre_animation,
            scale,
            layout: compiler.layout,
            queries,
        }
    }

    /// The index of a bone name in [`Rig::bones`], without regard to letter case.
    #[must_use]
    pub fn bone(&self, name: &str) -> Option<usize> {
        self.bones
            .iter()
            .position(|bone| bone.eq_ignore_ascii_case(name))
    }

    /// The index of an element of the `animate` list by its short name.
    #[must_use]
    pub fn entry(&self, short_name: &str) -> Option<usize> {
        self.animate
            .iter()
            .position(|entry| entry.short_name.eq_ignore_ascii_case(short_name))
    }

    /// The variable layout that each entity of this rig stores its variables in.
    #[must_use]
    pub fn layout(&self) -> &VariableLayout {
        &self.layout
    }
}

/// The clips and bone names that a rig compiled so far.
struct Resolved<'a> {
    clips: &'a mut Vec<Clip>,
    bones: &'a mut Vec<String>,
}

fn resolve(
    definition: &ClientEntityDefinition,
    library: &ClipLibrary,
    short_name: &str,
    resolved: Resolved<'_>,
    compiler: &mut FieldCompiler,
    context: &str,
) -> Option<AnimateTarget> {
    let Resolved { clips, bones } = resolved;
    let Some(identifier) = definition.animation(short_name) else {
        compiler.diagnostics.push(AnimationDiagnostic::new(
            DiagnosticKind::ContentError,
            context,
            "the short name is not in `animations`",
        ));
        return None;
    };
    if let Some(index) = clips.iter().position(|clip| clip.identifier == identifier) {
        return Some(AnimateTarget::Clip(index));
    }
    if let Some(object) = library.get(identifier) {
        let clip = Clip::compile(identifier, object, compiler, |bone| intern(bones, bone));
        clips.push(clip);
        return Some(AnimateTarget::Clip(clips.len() - 1));
    }
    let (kind, message) = if identifier.starts_with("controller.") {
        (
            DiagnosticKind::Unsupported,
            "animation controllers are not played yet",
        )
    } else {
        (DiagnosticKind::ContentError, "no clip has this identifier")
    };
    compiler.diagnostics.push(AnimationDiagnostic::new(
        kind,
        context,
        format!("{message}: {identifier}"),
    ));
    None
}

fn intern(bones: &mut Vec<String>, name: &str) -> usize {
    if let Some(index) = bones.iter().position(|bone| bone == name) {
        return index;
    }
    bones.push(name.into());
    bones.len() - 1
}

fn bindings(registry: &QueryRegistry) -> QueryBindings {
    let mut bindings = QueryBindings {
        count: registry.slot_count(),
        ..QueryBindings::default()
    };
    for (slot, key) in registry.slots() {
        let Some(declaration) = registry.declaration(key.declaration) else {
            continue;
        };
        let Some(query) = EntityQuery::from_declaration(&declaration.name, &key.arguments) else {
            continue;
        };
        match query {
            EntityQuery::DeltaTime => bindings.delta_time = Some(slot),
            EntityQuery::AnimTime => bindings.anim_time = Some(slot),
            EntityQuery::KeyFrameLerpTime => bindings.key_frame_lerp_time = Some(slot),
            query if query.is_animation_supplied() => {}
            query => bindings.host.push(HostQuery { slot, query }),
        }
    }
    bindings
}

impl Script {
    /// A script can skip a frame when no input changed, when it does not read a variable that
    /// it writes, and when no other expression writes its variables: then a skipped run is not
    /// visible.
    fn new(compiler: &FieldCompiler, expressions: Vec<Expression>) -> Self {
        let mut accesses = Accesses::default();
        for expression in &expressions {
            accesses.merge(expression.accesses());
        }
        let evaluation = if accesses.reads_own_write() || compiler.has_other_writer(&accesses) {
            Evaluation::EveryFrame
        } else {
            Evaluation::OnChange {
                queries: accesses.queries_read,
                variables: accesses.variables_read,
            }
        };
        Self {
            expressions,
            evaluation,
        }
    }
}
