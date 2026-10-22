use std::any::TypeId;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::Arc;

use mcrs_minecraft_core::ResourceLocation;

use super::ctx::{LoadCtx, Slot};
use super::declaration::{Declaration, EntrySource, Erased, Kind, Source};
use super::index::{self, Directories, SlotFiles};
use super::{Registries, Stage, Table, Tables, TagTables, network, tag_build};
use crate::diagnostics::{Diagnostic, Diagnostics, Missing};
use crate::order::{entry_file_order, identifier_order};
use crate::pack::{PackFiles, PackIndex, PackStack, StackedPack, VANILLA};
use crate::registry::Names;

pub(super) fn fail(mut diagnostics: Diagnostics, errors: Vec<Diagnostic>) -> Diagnostics {
    diagnostics.push_report(errors);
    diagnostics
}

pub(super) fn not_built(registry: &ResourceLocation) -> Diagnostic {
    Diagnostic::Declaration {
        registry: registry.clone(),
        message: "the registry was not built".to_owned(),
    }
}

pub(super) fn check_declarations(declarations: &[Declaration]) -> Vec<Diagnostic> {
    let mut errors = Vec::new();
    let mut keys = HashSet::new();
    let mut types = HashSet::new();
    let mut paths = HashSet::new();
    for declaration in declarations {
        let declaration = &declaration.inner;
        let registry = declaration.key().clone();
        let mut fresh = keys.insert(registry.clone());
        for &registry_type in declaration.types() {
            fresh &= types.insert(registry_type);
        }
        if !fresh {
            errors.push(Diagnostic::Declaration {
                registry,
                message: "the registry or its type is declared twice".to_owned(),
            });
            continue;
        }
        if !paths.insert(registry.path().to_owned()) {
            errors.push(Diagnostic::Declaration {
                registry,
                message: "another registry is declared with the same path".to_owned(),
            });
            continue;
        }
        if let Kind::Fixed(fixed) = declaration.kind() {
            let names = fixed.names();
            if names.iter().collect::<HashSet<_>>().len() != names.len() {
                errors.push(Diagnostic::Declaration {
                    registry,
                    message: "an entry name is listed twice".to_owned(),
                });
            }
        }
    }
    errors
}

pub(super) fn slots_by_type(slots: &[Slot<'_>]) -> HashMap<TypeId, usize> {
    slots
        .iter()
        .enumerate()
        .flat_map(|(at, slot)| slot.declaration.types().iter().map(move |&t| (t, at)))
        .collect()
}

pub(super) fn too_many(registry: &ResourceLocation, count: usize) -> Diagnostic {
    Diagnostic::TooManyEntries {
        registry: registry.clone(),
        count,
    }
}

pub(super) struct BuiltInPath<'d> {
    pub(super) pack: PackIndex,
    replaces_own_file: bool,
    set: usize,
    pub(super) name: &'d ResourceLocation,
    pub(super) relative: String,
}

pub(super) fn entry_sources<'f>(
    built_in: &[BuiltInPath<'_>],
    files: &mut SlotFiles<'f>,
    visibility: &index::Visibility<'_>,
) -> (Vec<ResourceLocation>, Vec<EntrySource<'f>>) {
    let mut entries = std::mem::take(&mut files.entries);
    for path in built_in {
        if visibility.hidden(path.pack, path.name.namespace(), &path.relative) {
            continue;
        }
        // A vanilla built-in replaces the game's file of the same name, which the jar still
        // ships; the built-in of any other pack yields to that pack's own file.
        if entries.get(path.name).is_none_or(|winner| {
            winner.pack < path.pack || (path.replaces_own_file && winner.pack == path.pack)
        }) {
            entries.insert(
                path.name.clone(),
                EntrySource {
                    pack: path.pack,
                    source: Source::BuiltIn { set: path.set },
                },
            );
        }
    }
    let mut entries: Vec<(ResourceLocation, EntrySource<'f>)> = entries.into_iter().collect();
    entries.sort_by(|a, b| entry_file_order(&a.0, &b.0));
    entries.into_iter().unzip()
}

pub(super) type BuiltInPaths<'d> = Vec<Vec<BuiltInPath<'d>>>;

pub(super) fn empty_slots(declarations: &[Declaration]) -> Vec<Slot<'_>> {
    let mut slots: Vec<Slot<'_>> = declarations
        .iter()
        .map(|d| Slot {
            declaration: d.inner.as_ref(),
            names: None,
            tags: None,
            table: None,
        })
        .collect();
    slots.sort_by(|a, b| {
        a.stage()
            .cmp(&b.stage())
            .then_with(|| identifier_order(a.key(), b.key()))
    });
    slots
}

pub(super) fn directories(slots: &[Slot<'_>]) -> Directories {
    let mut directories = Directories::default();
    for (at, slot) in slots.iter().enumerate() {
        let has_entries = matches!(slot.declaration.kind(), Kind::Data(_));
        directories.insert(slot.key(), at, has_entries);
    }
    directories
}

pub(super) fn built_in_paths<'d>(slots: &[Slot<'d>], stack: &PackStack) -> BuiltInPaths<'d> {
    slots
        .iter()
        .map(|slot| {
            let declaration = slot.declaration;
            let Kind::Data(data) = declaration.kind() else {
                return Vec::new();
            };
            let directory = declaration.key().path();
            let sets = data.built_ins().into_iter().enumerate();
            sets.filter_map(|(set, (pack, names))| {
                Some((set, stack.position(pack)?, pack == VANILLA, names))
            })
            .flat_map(|(set, pack, replaces_own_file, names)| {
                names.iter().map(move |name| BuiltInPath {
                    pack,
                    replaces_own_file,
                    set,
                    name,
                    relative: format!("{directory}/{}.json", name.path()),
                })
            })
            .collect()
        })
        .collect()
}

fn reuse(stage: Option<Stage>, slots: &mut [Slot<'_>], previous: &Registries) -> Vec<Diagnostic> {
    let mut errors = Vec::new();
    for slot in slots.iter_mut().filter(|s| s.stage() == stage) {
        let Some(table) = previous.table(slot.key()) else {
            errors.push(Diagnostic::Declaration {
                registry: slot.key().clone(),
                message: "the registry is missing from the registries being reloaded".to_owned(),
            });
            continue;
        };
        slot.names = Some(Arc::clone(&table.names));
        slot.table = Some(table.clone());
    }
    errors
}

fn name_entries<'f>(
    stage: Option<Stage>,
    slots: &mut [Slot<'_>],
    files: &mut [SlotFiles<'f>],
    built_in_paths: &BuiltInPaths<'_>,
    visibility: &index::Visibility<'_>,
) -> (Vec<(usize, Vec<EntrySource<'f>>)>, Vec<Diagnostic>) {
    let mut errors = Vec::new();
    let mut sources = Vec::new();
    let members = slots.iter_mut().zip(files.iter_mut()).enumerate();
    for (at, (slot, files)) in members.filter(|(_, (s, _))| s.stage() == stage) {
        let declaration = slot.declaration;
        match declaration.kind() {
            Kind::Fixed(fixed) => match Names::new(fixed.names().to_vec()) {
                Ok(names) => {
                    let names = Arc::new(names);
                    slot.table = Some(fixed.assemble(Arc::clone(&names)));
                    slot.names = Some(names);
                }
                Err(count) => errors.push(too_many(slot.key(), count)),
            },
            Kind::Data(_) => {
                let built_in = built_in_paths.get(at).map_or(&[][..], Vec::as_slice);
                let (names, entry_sources) = entry_sources(built_in, files, visibility);
                match Names::new(names) {
                    Ok(names) => {
                        slot.names = Some(Arc::new(names));
                        sources.push((at, entry_sources));
                    }
                    Err(count) => errors.push(too_many(slot.key(), count)),
                }
            }
        }
    }
    (sources, errors)
}

fn build_tags(
    stage: Option<Stage>,
    slots: &mut [Slot<'_>],
    files: &[SlotFiles<'_>],
    stack: &PackStack,
    previous: Option<&Registries>,
    diagnostics: &mut Diagnostics,
    errors: &mut Vec<Diagnostic>,
) -> BTreeMap<usize, Vec<Missing>> {
    let mut unresolved: BTreeMap<usize, Vec<Missing>> = BTreeMap::new();
    let mut resolution = Vec::new();
    let members = slots.iter_mut().zip(files).enumerate();
    for (at, (slot, files)) in members.filter(|(_, (s, _))| s.stage() == stage) {
        let Some(names) = slot.names.as_ref() else {
            errors.push(not_built(slot.key()));
            continue;
        };
        let outcome = tag_build::build(
            slot.key(),
            names,
            &files.tags,
            stack,
            previous.is_none() && stage.is_some(),
            previous
                .and_then(|p| p.tag_table(slot.key()))
                .map(Arc::as_ref),
        );
        diagnostics.extend(outcome.file_diagnostics);
        resolution.extend(outcome.resolution_diagnostics);
        if !outcome.unresolved.is_empty() {
            unresolved.entry(at).or_default().extend(outcome.unresolved);
        }
        match outcome.table {
            Ok(table) => slot.tags = Some(Arc::new(table)),
            Err(count) => errors.push(Diagnostic::TooManyTags {
                registry: slot.key().clone(),
                count,
            }),
        }
    }
    diagnostics.extend(resolution);
    unresolved
}

pub(super) fn decode(
    stage: Option<Stage>,
    slots: &mut [Slot<'_>],
    by_type: &HashMap<TypeId, usize>,
    sources: Vec<(usize, Vec<EntrySource<'_>>)>,
    stack: &PackStack,
    errors: &mut Vec<Diagnostic>,
) -> Vec<(usize, Missing)> {
    let ctx = LoadCtx::new(slots, by_type, stage);
    let mut built: Vec<(usize, Table)> = Vec::new();
    for (at, entry_sources) in sources {
        let Some(slot) = slots.get(at) else {
            continue;
        };
        let (Kind::Data(data), Some(names)) = (slot.declaration.kind(), slot.names.as_ref()) else {
            errors.push(not_built(slot.key()));
            continue;
        };
        match data.build(&ctx, names, entry_sources) {
            Ok(table) => built.push((at, table)),
            Err(failures) => {
                let reported = failures.into_iter().filter(|f| !f.reported_as_unresolved);
                errors.extend(reported.map(|failure| {
                    Diagnostic::Entry {
                        registry: slot.key().clone(),
                        entry: failure.entry,
                        pack: stack
                            .get(failure.pack)
                            .map(|pack| StackedPack::id(pack).to_owned()),
                        file: failure.file,
                        field: failure.field,
                        message: failure.message,
                    }
                }));
            }
        }
    }
    let unresolved = ctx.into_unresolved();
    for (at, table) in built {
        if let Some(slot) = slots.get_mut(at) {
            slot.table = Some(table);
        }
    }
    unresolved
}

fn validate(
    stage: Option<Stage>,
    slots: &[Slot<'_>],
    by_type: &HashMap<TypeId, usize>,
) -> Vec<Diagnostic> {
    let mut errors = Vec::new();
    let ctx = LoadCtx::new(slots, by_type, stage);
    for slot in slots.iter().filter(|s| s.stage() == stage) {
        let Kind::Data(data) = slot.declaration.kind() else {
            continue;
        };
        let Some(table) = slot.table.as_ref() else {
            errors.push(not_built(slot.key()));
            continue;
        };
        let registry = slot.key().clone();
        if data.must_not_be_empty() && table.names.len() == 0 {
            errors.push(Diagnostic::EmptyRegistry { registry });
        } else if let Err(message) = data.validate(table, &ctx) {
            errors.push(Diagnostic::Validation { registry, message });
        }
    }
    errors
}

pub(super) fn assemble(
    slots: Vec<Slot<'_>>,
    stack: Arc<PackStack>,
    previous: Option<&Registries>,
) -> Result<Registries, Vec<Diagnostic>> {
    let mut world = Tables::default();
    let mut reloadable = Tables::default();
    let mut tags = TagTables::default();
    let mut keys = HashMap::new();
    let mut errors = Vec::new();
    for slot in slots {
        let stage = slot.stage();
        let (Some(table), Some(tag_table)) = (slot.table, slot.tags) else {
            errors.push(not_built(slot.declaration.key()));
            continue;
        };
        let key = slot.declaration.key();
        for &registry_type in slot.declaration.types() {
            keys.insert(registry_type, key.clone());
        }
        tags.insert(key.clone(), tag_table);
        match stage {
            Some(Stage::Reloadable) => reloadable.insert(key.clone(), table),
            _ => world.insert(key.clone(), table),
        };
    }
    if !errors.is_empty() {
        return Err(errors);
    }
    Ok(Registries {
        world: previous.map_or_else(|| Arc::new(world), |p| Arc::clone(&p.world)),
        reloadable: Arc::new(reloadable),
        tags: Arc::new(tags),
        keys: Arc::new(keys),
        world_stack: previous.map_or_else(|| Arc::clone(&stack), |p| Arc::clone(&p.world_stack)),
        reload_stack: stack,
        generation: previous.map_or(0, |p| p.generation.saturating_add(1)),
        network: Arc::default(),
    })
}

pub(super) fn report_unresolved(
    unresolved: BTreeMap<usize, Vec<Missing>>,
    slots: &[Slot<'_>],
    errors: &mut Vec<Diagnostic>,
) {
    for (at, missing) in unresolved {
        if let Some(slot) = slots.get(at) {
            errors.push(Diagnostic::Unresolved {
                registry: slot.key().clone(),
                missing,
            });
        }
    }
}

type Pending<'f> = (
    Vec<(usize, Vec<EntrySource<'f>>)>,
    BTreeMap<usize, Vec<Missing>>,
);

fn run_stage(
    stage: Stage,
    slots: &mut [Slot<'_>],
    by_type: &HashMap<TypeId, usize>,
    stack: &PackStack,
    (sources, mut unresolved): Pending<'_>,
) -> Vec<Diagnostic> {
    let mut errors = Vec::new();
    let missing = decode(Some(stage), slots, by_type, sources, stack, &mut errors);
    for (at, missing) in missing {
        unresolved.entry(at).or_default().push(missing);
    }
    report_unresolved(unresolved, slots, &mut errors);
    if errors.is_empty() {
        errors = validate(Some(stage), slots, by_type);
    }
    errors
}

pub fn load(
    declarations: &[Declaration],
    stack: Arc<PackStack>,
    files: &PackFiles,
    previous: Option<&Registries>,
) -> Result<(Registries, Diagnostics), Diagnostics> {
    let mut diagnostics = Diagnostics::default();
    let errors = check_declarations(declarations);
    if !errors.is_empty() {
        return Err(fail(diagnostics, errors));
    }

    let mut slots = empty_slots(declarations);
    let by_type = slots_by_type(&slots);

    let built_in_paths = built_in_paths(&slots, &stack);
    let mut found = Vec::new();
    let candidates = index::candidates(&stack, files, &directories(&slots), &mut found);
    let tested: Vec<(PackIndex, &str, &str)> = candidates
        .iter()
        .map(index::Candidate::tested)
        .chain(
            built_in_paths
                .iter()
                .flatten()
                .map(|path| (path.pack, path.name.namespace(), path.relative.as_str())),
        )
        .collect();
    let visibility = index::Visibility::new(&stack, &tested, &mut found);
    let mut slot_files = index::assign(candidates, &visibility, slots.len());
    diagnostics.extend(found);

    const STAGES: [Option<Stage>; 4] = [
        None,
        Some(Stage::World),
        Some(Stage::Dimension),
        Some(Stage::Reloadable),
    ];
    for stage in STAGES {
        let reused = previous.filter(|_| stage < Some(Stage::Reloadable));
        let (sources, mut errors) = match reused {
            Some(previous) => (Vec::new(), reuse(stage, &mut slots, previous)),
            None => name_entries(
                stage,
                &mut slots,
                &mut slot_files,
                &built_in_paths,
                &visibility,
            ),
        };
        if !errors.is_empty() {
            return Err(fail(diagnostics, errors));
        }

        let unresolved = build_tags(
            stage,
            &mut slots,
            &slot_files,
            &stack,
            previous,
            &mut diagnostics,
            &mut errors,
        );
        if !errors.is_empty() {
            return Err(fail(diagnostics, errors));
        }
        match stage.filter(|_| reused.is_none()) {
            Some(stage) => {
                let pending = (sources, unresolved);
                errors = run_stage(stage, &mut slots, &by_type, &stack, pending);
            }
            None => report_unresolved(unresolved, &slots, &mut errors),
        }
        if !errors.is_empty() {
            return Err(fail(diagnostics, errors));
        }
    }

    let mut registries = match assemble(slots, stack, previous) {
        Ok(registries) => registries,
        Err(errors) => return Err(fail(diagnostics, errors)),
    };
    match network::snapshot(
        &declarations
            .iter()
            .map(|d| &*d.inner)
            .collect::<Vec<&dyn Erased>>(),
        &registries,
    ) {
        Ok(network) => {
            registries.network = Arc::new(network);
            Ok((registries, diagnostics))
        }
        Err(errors) => Err(fail(diagnostics, errors)),
    }
}
