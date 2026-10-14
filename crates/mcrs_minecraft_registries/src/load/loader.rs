use std::any::TypeId;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::Arc;

use mcrs_minecraft_core::ResourceLocation;

use super::ctx::{LoadCtx, Slot};
use super::declaration::{Declaration, EntrySource, Erased, Kind, Source};
use super::index::{self, Directories, SlotFiles};
use super::{RELOADABLE_RANK, Registries, STATIC_RANK, Table, Tables, TagSets, tag_build};
use crate::diagnostics::{Diagnostic, Diagnostics, Missing};
use crate::order::{entry_file_order, identifier_order};
use crate::pack::{PackFiles, PackIndex, PackStack, StackedPack, VANILLA};
use crate::registry::Names;

fn fail(mut diagnostics: Diagnostics, errors: Vec<Diagnostic>) -> Diagnostics {
    diagnostics.push_report(errors);
    diagnostics
}

fn not_built(registry: &ResourceLocation) -> Diagnostic {
    Diagnostic::Declaration {
        registry: registry.clone(),
        message: "the registry was not built".to_owned(),
    }
}

fn check_declarations(declarations: &[Declaration]) -> Vec<Diagnostic> {
    let mut errors = Vec::new();
    let mut keys = HashSet::new();
    let mut types = HashSet::new();
    for declaration in declarations {
        let declaration = &declaration.inner;
        let registry = declaration.key().clone();
        if !keys.insert(registry.clone()) || !types.insert(declaration.registry_type()) {
            errors.push(Diagnostic::Declaration {
                registry,
                message: "the registry or its type is declared twice".to_owned(),
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

fn too_many(registry: &ResourceLocation, count: usize) -> Diagnostic {
    Diagnostic::TooManyEntries {
        registry: registry.clone(),
        count,
    }
}

fn entry_sources<'f>(
    built_in: &[(&ResourceLocation, String)],
    files: &mut SlotFiles<'f>,
    vanilla: Option<PackIndex>,
    visibility: &index::Visibility,
) -> (Vec<ResourceLocation>, Vec<EntrySource<'f>>) {
    let mut entries = std::mem::take(&mut files.entries);
    if let Some(vanilla) = vanilla {
        for (name, relative) in built_in {
            if visibility.hidden(vanilla, name.namespace(), relative) {
                continue;
            }
            if entries
                .get(*name)
                .is_none_or(|winner| winner.pack < vanilla)
            {
                entries.insert(
                    (*name).clone(),
                    EntrySource {
                        pack: vanilla,
                        source: Source::BuiltIn,
                    },
                );
            }
        }
    }
    let mut entries: Vec<(ResourceLocation, EntrySource<'f>)> = entries.into_iter().collect();
    entries.sort_by(|a, b| entry_file_order(&a.0, &b.0));
    entries.into_iter().unzip()
}

type Work<'d, 'f> = (&'d dyn Erased, SlotFiles<'f>);
type BuiltInPaths<'d> = Vec<Vec<(&'d ResourceLocation, String)>>;

fn ordered(declarations: &[Declaration]) -> Vec<&dyn Erased> {
    let mut ordered: Vec<&dyn Erased> = declarations.iter().map(|d| d.inner.as_ref()).collect();
    ordered.sort_by(|a, b| {
        a.kind()
            .rank()
            .cmp(&b.kind().rank())
            .then_with(|| identifier_order(a.key(), b.key()))
    });
    ordered
}

fn empty_slots(ordered: &[&dyn Erased]) -> Vec<Slot> {
    ordered
        .iter()
        .map(|d| Slot {
            key: d.key().clone(),
            rank: d.kind().rank(),
            names: None,
            tags: None,
            table: None,
        })
        .collect()
}

fn directories(ordered: &[&dyn Erased]) -> Directories {
    let mut directories = Directories::default();
    for (at, declaration) in ordered.iter().enumerate() {
        let has_entries = matches!(declaration.kind(), Kind::Data(_));
        directories.insert(declaration.key(), at, has_entries);
    }
    directories
}

fn built_in_paths<'d>(ordered: &[&'d dyn Erased], vanilla: Option<PackIndex>) -> BuiltInPaths<'d> {
    ordered
        .iter()
        .map(|d| match (vanilla, d.kind()) {
            (Some(_), Kind::Data(data)) => {
                let directory = Directories::directory(d.key());
                data.built_in_names()
                    .iter()
                    .map(|name| (name, format!("{directory}/{}.json", name.path())))
                    .collect()
            }
            _ => Vec::new(),
        })
        .collect()
}

fn reuse(
    rank: u8,
    slots: &mut [Slot],
    work: &[Work<'_, '_>],
    previous: &Registries,
) -> Vec<Diagnostic> {
    let mut errors = Vec::new();
    for (slot, (declaration, _)) in slots.iter_mut().zip(work).filter(|(s, _)| s.rank == rank) {
        let Some(table) = previous.table(declaration.registry_type()) else {
            errors.push(Diagnostic::Declaration {
                registry: slot.key.clone(),
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
    rank: u8,
    slots: &mut [Slot],
    work: &mut [Work<'_, 'f>],
    built_in_paths: &BuiltInPaths<'_>,
    vanilla: Option<PackIndex>,
    visibility: &index::Visibility,
) -> (Vec<(usize, Vec<EntrySource<'f>>)>, Vec<Diagnostic>) {
    let mut errors = Vec::new();
    let mut sources = Vec::new();
    let members = slots.iter_mut().zip(work.iter_mut()).enumerate();
    for (at, (slot, (declaration, files))) in members.filter(|(_, (s, _))| s.rank == rank) {
        match declaration.kind() {
            Kind::Fixed(fixed) => match Names::new(fixed.names().to_vec()) {
                Ok(names) => {
                    let names = Arc::new(names);
                    slot.table = Some(fixed.assemble(Arc::clone(&names)));
                    slot.names = Some(names);
                }
                Err(count) => errors.push(too_many(&slot.key, count)),
            },
            Kind::Data(_) => {
                let built_in = built_in_paths.get(at).map_or(&[][..], Vec::as_slice);
                let (names, entry_sources) = entry_sources(built_in, files, vanilla, visibility);
                match Names::new(names) {
                    Ok(names) => {
                        slot.names = Some(Arc::new(names));
                        sources.push((at, entry_sources));
                    }
                    Err(count) => errors.push(too_many(&slot.key, count)),
                }
            }
        }
    }
    (sources, errors)
}

fn build_tags(
    rank: u8,
    slots: &mut [Slot],
    work: &[Work<'_, '_>],
    stack: &PackStack,
    previous: Option<&Registries>,
    diagnostics: &mut Diagnostics,
    errors: &mut Vec<Diagnostic>,
) -> BTreeMap<usize, Vec<Missing>> {
    let mut unresolved: BTreeMap<usize, Vec<Missing>> = BTreeMap::new();
    let mut resolution = Vec::new();
    let members = slots.iter_mut().zip(work).enumerate();
    for (at, (slot, (declaration, files))) in members.filter(|(_, (s, _))| s.rank == rank) {
        let Some(names) = slot.names.as_ref() else {
            errors.push(not_built(&slot.key));
            continue;
        };
        let outcome = tag_build::build(
            &slot.key,
            names,
            &files.tags,
            stack,
            previous.is_none() && rank != STATIC_RANK,
            previous
                .and_then(|p| p.tag_table(declaration.registry_type()))
                .map(Arc::as_ref),
        );
        diagnostics.extend(outcome.file_diagnostics);
        resolution.extend(outcome.resolution_diagnostics);
        if !outcome.unresolved.is_empty() {
            unresolved.entry(at).or_default().extend(outcome.unresolved);
        }
        slot.tags = Some(Arc::new(outcome.table));
    }
    diagnostics.extend(resolution);
    unresolved
}

fn decode(
    rank: u8,
    slots: &mut [Slot],
    work: &[Work<'_, '_>],
    by_type: &HashMap<TypeId, usize>,
    sources: Vec<(usize, Vec<EntrySource<'_>>)>,
    stack: &PackStack,
    errors: &mut Vec<Diagnostic>,
) -> Vec<(usize, Missing)> {
    let ctx = LoadCtx::new(slots, by_type, rank);
    let mut built: Vec<(usize, Table)> = Vec::new();
    for (at, entry_sources) in sources {
        let (Some(slot), Some((declaration, _))) = (slots.get(at), work.get(at)) else {
            continue;
        };
        let (Kind::Data(data), Some(names)) = (declaration.kind(), slot.names.as_ref()) else {
            errors.push(not_built(&slot.key));
            continue;
        };
        match data.build(&ctx, names, entry_sources) {
            Ok(table) => built.push((at, table)),
            Err(failures) => {
                let reported = failures.into_iter().filter(|f| !f.reported_as_unresolved);
                errors.extend(reported.map(|failure| {
                    Diagnostic::Entry {
                        registry: slot.key.clone(),
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
    rank: u8,
    slots: &[Slot],
    work: &[Work<'_, '_>],
    by_type: &HashMap<TypeId, usize>,
) -> Vec<Diagnostic> {
    let mut errors = Vec::new();
    let ctx = LoadCtx::new(slots, by_type, rank);
    for (slot, (declaration, _)) in slots.iter().zip(work).filter(|(s, _)| s.rank == rank) {
        let Kind::Data(data) = declaration.kind() else {
            continue;
        };
        let Some(table) = slot.table.as_ref() else {
            errors.push(not_built(&slot.key));
            continue;
        };
        let registry = slot.key.clone();
        if data.must_not_be_empty() && table.names.len() == 0 {
            errors.push(Diagnostic::EmptyRegistry { registry });
        } else if let Err(message) = data.validate(table, &ctx) {
            errors.push(Diagnostic::Validation { registry, message });
        }
    }
    errors
}

fn assemble(
    slots: Vec<Slot>,
    work: Vec<Work<'_, '_>>,
    stack: Arc<PackStack>,
    previous: Option<&Registries>,
) -> Result<Registries, Vec<Diagnostic>> {
    let mut world = Tables::default();
    let mut reloadable = Tables::default();
    let mut tags = TagSets::default();
    let mut errors = Vec::new();
    for (slot, (declaration, _)) in slots.into_iter().zip(work) {
        let (Some(table), Some(tag_table)) = (slot.table, slot.tags) else {
            errors.push(not_built(&slot.key));
            continue;
        };
        let registry_type = declaration.registry_type();
        match slot.rank {
            RELOADABLE_RANK => reloadable.by_type.insert(registry_type, table),
            _ => world.by_type.insert(registry_type, table),
        };
        tags.by_type
            .insert(registry_type, declaration.wrap_tags(tag_table));
    }
    if !errors.is_empty() {
        return Err(errors);
    }
    Ok(Registries {
        world: previous.map_or_else(|| Arc::new(world), |p| Arc::clone(&p.world)),
        reloadable: Arc::new(reloadable),
        tags: Arc::new(tags),
        world_stack: previous.map_or_else(|| Arc::clone(&stack), |p| Arc::clone(&p.world_stack)),
        reload_stack: stack,
        generation: previous.map_or(0, |p| p.generation.saturating_add(1)),
    })
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

    let ordered = ordered(declarations);
    let by_type: HashMap<TypeId, usize> = ordered
        .iter()
        .enumerate()
        .map(|(at, d)| (d.registry_type(), at))
        .collect();
    let mut slots = empty_slots(&ordered);

    let vanilla = stack.position(VANILLA);
    let built_in_paths = built_in_paths(&ordered, vanilla);
    let mut found = Vec::new();
    let candidates = index::candidates(&stack, files, &directories(&ordered), &mut found);
    let tested: Vec<(PackIndex, &str, &str)> = candidates
        .iter()
        .map(index::Candidate::tested)
        .chain(vanilla.into_iter().flat_map(|vanilla| {
            built_in_paths
                .iter()
                .flatten()
                .map(move |(name, relative)| (vanilla, name.namespace(), relative.as_str()))
        }))
        .collect();
    let visibility = index::Visibility::new(&stack, &tested, &mut found);
    let slot_files = index::assign(candidates, &visibility, slots.len());
    let mut work: Vec<Work<'_, '_>> = ordered.iter().copied().zip(slot_files).collect();
    diagnostics.extend(found);

    for rank in STATIC_RANK..=RELOADABLE_RANK {
        let reused = previous.filter(|_| rank < RELOADABLE_RANK);
        let (sources, mut errors) = match reused {
            Some(previous) => (Vec::new(), reuse(rank, &mut slots, &work, previous)),
            None => name_entries(
                rank,
                &mut slots,
                &mut work,
                &built_in_paths,
                vanilla,
                &visibility,
            ),
        };
        if !errors.is_empty() {
            return Err(fail(diagnostics, errors));
        }

        let mut unresolved = build_tags(
            rank,
            &mut slots,
            &work,
            &stack,
            previous,
            &mut diagnostics,
            &mut errors,
        );
        let decoded = reused.is_none() && rank != STATIC_RANK;
        if decoded {
            let missing = decode(
                rank,
                &mut slots,
                &work,
                &by_type,
                sources,
                &stack,
                &mut errors,
            );
            for (at, missing) in missing {
                unresolved.entry(at).or_default().push(missing);
            }
        }
        for (at, missing) in unresolved {
            if let Some(slot) = slots.get(at) {
                errors.push(Diagnostic::Unresolved {
                    registry: slot.key.clone(),
                    missing,
                });
            }
        }
        if !errors.is_empty() {
            return Err(fail(diagnostics, errors));
        }

        if decoded {
            let errors = validate(rank, &slots, &work, &by_type);
            if !errors.is_empty() {
                return Err(fail(diagnostics, errors));
            }
        }
    }

    match assemble(slots, work, stack, previous) {
        Ok(registries) => Ok((registries, diagnostics)),
        Err(errors) => Err(fail(diagnostics, errors)),
    }
}
