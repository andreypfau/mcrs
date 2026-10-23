use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::Arc;

use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_datapack::{PackFiles, PackIndex, PackStack, Visibility};

use super::ctx::{LoadCtx, Slot};
use super::declaration::{Declaration, EntrySource, Kind};
use super::index::{self, Directories, SlotFiles};
use super::tag_build;
use crate::network;
use crate::registries::{Erased, Registries, Stage, Table};
use crate::diagnostics::{Diagnostic, Diagnostics, Missing, Referrer};
use crate::order::{entry_file_order, identifier_order};
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
    let mut paths = HashSet::new();
    for declaration in declarations {
        let registry = declaration.key().clone();
        if !keys.insert(registry.clone()) {
            errors.push(Diagnostic::Declaration {
                registry,
                message: "the registry is declared twice".to_owned(),
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
        if let Kind::Fixed(names) = &declaration.kind
            && names.iter().collect::<HashSet<_>>().len() != names.len()
        {
            errors.push(Diagnostic::Declaration {
                registry,
                message: "an entry name is listed twice".to_owned(),
            });
        }
    }
    errors
}

pub(super) fn slots_by_key(slots: &[Slot<'_>]) -> HashMap<ResourceLocation, usize> {
    slots
        .iter()
        .enumerate()
        .map(|(at, slot)| (slot.key().clone(), at))
        .collect()
}

pub(super) fn too_many(registry: &ResourceLocation, count: usize) -> Diagnostic {
    Diagnostic::TooManyEntries {
        registry: registry.clone(),
        count,
    }
}

fn entry_sources<'f>(files: &mut SlotFiles<'f>) -> (Vec<ResourceLocation>, Vec<EntrySource<'f>>) {
    let mut entries: Vec<(ResourceLocation, EntrySource<'f>)> =
        std::mem::take(&mut files.entries).into_iter().collect();
    entries.sort_by(|a, b| entry_file_order(&a.0, &b.0));
    entries.into_iter().unzip()
}

pub(super) fn empty_slots(declarations: &[Declaration]) -> Vec<Slot<'_>> {
    let mut slots: Vec<Slot<'_>> = declarations
        .iter()
        .map(|declaration| Slot {
            declaration,
            names: None,
            tags: None,
            registry: None,
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
        let has_entries = !matches!(slot.declaration.kind, Kind::Fixed(_));
        directories.insert(slot.key(), at, has_entries);
    }
    directories
}

fn reuse(stage: Stage, slots: &mut [Slot<'_>], previous: &Registries) -> Vec<Diagnostic> {
    let mut errors = Vec::new();
    for slot in slots.iter_mut().filter(|s| s.stage() == stage) {
        let Some(table) = previous.table(slot.key().as_str()) else {
            errors.push(Diagnostic::Declaration {
                registry: slot.key().clone(),
                message: "the registry is missing from the registries being reloaded".to_owned(),
            });
            continue;
        };
        slot.names = Some(Arc::clone(&table.names));
        slot.registry = table.registry.clone();
    }
    errors
}

fn name_entries<'f>(
    stage: Stage,
    slots: &mut [Slot<'_>],
    files: &mut [SlotFiles<'f>],
) -> (Vec<(usize, Vec<EntrySource<'f>>)>, Vec<Diagnostic>) {
    let mut errors = Vec::new();
    let mut sources = Vec::new();
    let members = slots.iter_mut().zip(files.iter_mut()).enumerate();
    for (at, (slot, files)) in members.filter(|(_, (s, _))| s.stage() == stage) {
        let named = match &slot.declaration.kind {
            Kind::Fixed(names) => Names::new(names.clone(), Vec::new()),
            Kind::Names | Kind::Data(_) => {
                let (names, entry_sources) = entry_sources(files);
                let origins = entry_sources.iter().map(|source| source.pack).collect();
                if slot.declaration.data_part().is_some() {
                    sources.push((at, entry_sources));
                }
                Names::new(names, origins)
            }
        };
        match named {
            Ok(names) => slot.names = Some(Arc::new(names)),
            Err(count) => errors.push(too_many(slot.key(), count)),
        }
    }
    (sources, errors)
}

fn build_tags(
    stage: Stage,
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
            previous.is_none() && stage != Stage::Static,
            previous
                .and_then(|p| p.table(slot.key().as_str()))
                .map(|table| table.tags.as_ref()),
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
    stage: Stage,
    slots: &mut [Slot<'_>],
    by_key: &HashMap<ResourceLocation, usize>,
    stack: &PackStack,
    (sources, mut unresolved): Pending<'_>,
) -> Vec<Diagnostic> {
    let mut errors = Vec::new();
    let ctx = LoadCtx::new(slots, by_key, stage);
    let mut built: Vec<(usize, Erased)> = Vec::new();
    for (at, entry_sources) in sources {
        let Some(slot) = slots.get(at) else {
            continue;
        };
        let (Some(data), Some(names)) = (slot.declaration.data_part(), slot.names.as_ref()) else {
            errors.push(not_built(slot.key()));
            continue;
        };
        let failures = match data.build(&ctx, names, entry_sources) {
            Ok(registry) => {
                built.push((at, registry));
                continue;
            }
            Err(failures) => failures,
        };
        for failure in failures {
            match failure.error.into_missing() {
                Ok(missing) => {
                    if let Some(&holder) = by_key.get(&missing.registry) {
                        unresolved.entry(holder).or_default().push(Missing {
                            name: missing.name,
                            tag: missing.tag,
                            referrer: Referrer::Entry {
                                registry: slot.key().clone(),
                                entry: failure.entry,
                            },
                        });
                    }
                }
                Err(error) => errors.push(Diagnostic::Entry {
                    registry: slot.key().clone(),
                    entry: failure.entry,
                    pack: stack.get(failure.pack).map(|pack| pack.id().to_owned()),
                    file: failure.file,
                    field: error.path().map(str::to_owned),
                    message: error.message().to_owned(),
                }),
            }
        }
    }
    for (at, registry) in built {
        if let Some(slot) = slots.get_mut(at) {
            slot.registry = Some(registry);
        }
    }
    report_unresolved(unresolved, slots, &mut errors);
    errors
}

pub(super) fn empty_registry(slot: &Slot<'_>) -> Option<Diagnostic> {
    let (data, names) = (slot.declaration.data_part()?, slot.names.as_ref()?);
    (data.must_not_be_empty() && names.len() == 0).then(|| Diagnostic::EmptyRegistry {
        registry: slot.key().clone(),
    })
}

fn validate(
    stage: Stage,
    slots: &[Slot<'_>],
    by_key: &HashMap<ResourceLocation, usize>,
    stack: &PackStack,
) -> Vec<Diagnostic> {
    let mut errors = Vec::new();
    let ctx = LoadCtx::new(slots, by_key, stage);
    for slot in slots.iter().filter(|s| s.stage() == stage) {
        let Some(data) = slot.declaration.data_part() else {
            continue;
        };
        let (Some(names), Some(registry)) = (&slot.names, &slot.registry) else {
            errors.push(not_built(slot.key()));
            continue;
        };
        if let Some(empty) = empty_registry(slot) {
            errors.push(empty);
            continue;
        }
        for (at, message) in data.validate(registry, &ctx) {
            let number = u16::try_from(at).ok();
            let entry = number.and_then(|number| names.name(number));
            errors.push(match entry {
                Some(entry) => Diagnostic::Entry {
                    registry: slot.key().clone(),
                    entry: entry.clone(),
                    pack: (number.and_then(|number| names.origin(number)))
                        .and_then(|pack| stack.get(pack))
                        .map(|pack| pack.id().to_owned()),
                    file: None,
                    field: None,
                    message,
                },
                None => Diagnostic::Declaration {
                    registry: slot.key().clone(),
                    message: format!("the validator names entry {at}, which is absent: {message}"),
                },
            });
        }
    }
    errors
}

pub(super) fn assemble(
    slots: Vec<Slot<'_>>,
    stack: Arc<PackStack>,
    previous: Option<&Registries>,
) -> Result<Registries, Vec<Diagnostic>> {
    let mut tables = HashMap::new();
    let mut errors = Vec::new();
    for slot in slots {
        let stage = slot.stage();
        let key = slot.key().clone();
        let built = slot.registry.is_some() || slot.declaration.data_part().is_none();
        let (Some(names), Some(tags), true) = (slot.names, slot.tags, built) else {
            errors.push(not_built(&key));
            continue;
        };
        let table = Table {
            stage,
            names,
            tags,
            registry: slot.registry,
        };
        tables.insert(key, table);
    }
    if !errors.is_empty() {
        return Err(errors);
    }
    Ok(Registries {
        tables: Arc::new(tables),
        world_stack: previous.map_or_else(|| Arc::clone(&stack), |p| Arc::clone(&p.world_stack)),
        reload_stack: stack,
        network: Arc::default(),
    })
}

fn report_unresolved(
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

pub(super) type Pending<'f> = (
    Vec<(usize, Vec<EntrySource<'f>>)>,
    BTreeMap<usize, Vec<Missing>>,
);

fn run_stage(
    stage: Stage,
    slots: &mut [Slot<'_>],
    by_key: &HashMap<ResourceLocation, usize>,
    stack: &PackStack,
    pending: Pending<'_>,
) -> Vec<Diagnostic> {
    let errors = decode(stage, slots, by_key, stack, pending);
    if errors.is_empty() {
        validate(stage, slots, by_key, stack)
    } else {
        errors
    }
}

/// Loads `declarations` over `stack` from `files`, whose values move into the registries.
pub fn load(
    declarations: &[Declaration],
    stack: Arc<PackStack>,
    mut files: PackFiles,
    previous: Option<&Registries>,
) -> Result<(Registries, Diagnostics), Diagnostics> {
    let mut diagnostics = Diagnostics::default();
    let errors = check_declarations(declarations);
    if !errors.is_empty() {
        return Err(fail(diagnostics, errors));
    }

    let mut slots = empty_slots(declarations);
    let by_key = slots_by_key(&slots);

    let mut found = Vec::new();
    let candidates = index::candidates(&stack, &mut files, &directories(&slots), &mut found);
    let tested: Vec<(PackIndex, &str, &str)> =
        candidates.iter().map(index::Candidate::tested).collect();
    let (visibility, ignored) = Visibility::new(&stack, &tested);
    let mut slot_files = index::assign(candidates, &visibility, slots.len());
    diagnostics.extend(found);
    diagnostics.extend(ignored.into_iter().map(Diagnostic::Pack));

    for stage in Stage::ALL {
        let reused = previous.filter(|_| stage < Stage::Reloadable);
        let (sources, mut errors) = match reused {
            Some(previous) => (Vec::new(), reuse(stage, &mut slots, previous)),
            None => name_entries(stage, &mut slots, &mut slot_files),
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
        errors = match reused {
            Some(_) => {
                let mut errors = Vec::new();
                report_unresolved(unresolved, &slots, &mut errors);
                errors
            }
            None => run_stage(stage, &mut slots, &by_key, &stack, (sources, unresolved)),
        };
        if !errors.is_empty() {
            return Err(fail(diagnostics, errors));
        }
    }

    let mut registries = match assemble(slots, stack, previous) {
        Ok(registries) => registries,
        Err(errors) => return Err(fail(diagnostics, errors)),
    };
    match network::snapshot(declarations, &registries) {
        Ok(network) => {
            registries.network = Arc::new(network);
            Ok((registries, diagnostics))
        }
        Err(errors) => Err(fail(diagnostics, errors)),
    }
}
