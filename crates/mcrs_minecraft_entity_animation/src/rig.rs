use mcrs_minecraft_pack_file::{
    ClientEntityFile, DiagnosticKind, FieldCompiler, PackDiagnostic, SCALE_FIELDS,
};
use mcrs_molang::{Accesses, Expression, QueryRegistry, QuerySlot, VariableLayout, VariableSlot};

use crate::clip::Clip;
use crate::clip_library::ClipLibrary;
use crate::entity_query::EntityQuery;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum AnimateTarget {
    Clip(usize),
}

#[derive(Clone, Debug, PartialEq)]
pub struct AnimateEntry {
    pub short_name: String,
    pub weight: Expression,
    pub target: AnimateTarget,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct HostQuery {
    pub slot: QuerySlot,
    pub query: EntityQuery,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct QueryBindings {
    pub host: Vec<HostQuery>,
    pub delta_time: Option<QuerySlot>,
    pub anim_time: Option<QuerySlot>,
    pub key_frame_lerp_time: Option<QuerySlot>,
    pub count: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Evaluation {
    EveryFrame,
    OnChange {
        queries: Vec<QuerySlot>,
        variables: Vec<VariableSlot>,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Script {
    pub expressions: Vec<Expression>,
    pub evaluation: Evaluation,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Rig {
    pub identifier: String,
    pub bones: Vec<String>,
    pub clips: Vec<Clip>,
    pub animate: Vec<AnimateEntry>,
    pub(crate) initialize: Option<Expression>,
    pub(crate) pre_animation: Option<Script>,
    pub(crate) scale: Script,
    pub(crate) layout: VariableLayout,
    pub(crate) queries: QueryBindings,
}

impl Rig {
    pub fn compile(
        definition: &ClientEntityFile,
        library: &ClipLibrary,
        diagnostics: &mut Vec<PackDiagnostic>,
    ) -> Self {
        let mut compiler = FieldCompiler::new(EntityQuery::registry());
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
            .zip(SCALE_FIELDS)
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

    #[must_use]
    pub fn bone(&self, name: &str) -> Option<usize> {
        self.bones
            .iter()
            .position(|bone| bone.eq_ignore_ascii_case(name))
    }

    #[must_use]
    pub fn entry(&self, short_name: &str) -> Option<usize> {
        self.animate
            .iter()
            .position(|entry| entry.short_name.eq_ignore_ascii_case(short_name))
    }

    #[must_use]
    pub fn layout(&self) -> &VariableLayout {
        &self.layout
    }
}

struct Resolved<'a> {
    clips: &'a mut Vec<Clip>,
    bones: &'a mut Vec<String>,
}

fn resolve(
    definition: &ClientEntityFile,
    library: &ClipLibrary,
    short_name: &str,
    resolved: Resolved<'_>,
    compiler: &mut FieldCompiler,
    context: &str,
) -> Option<AnimateTarget> {
    let Resolved { clips, bones } = resolved;
    let Some(identifier) = definition.animation(short_name) else {
        compiler.report(
            DiagnosticKind::ContentError,
            context,
            "the short name is not in `animations`",
        );
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
    compiler.report(kind, context, format!("{message}: {identifier}"));
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
