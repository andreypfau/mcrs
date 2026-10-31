use mcrs_minecraft_resource::ResourceLocationBuf;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::Arc;

use mcrs_minecraft_datapack::{
    PackContent, PackDiagnostic, PackFile, PackFiles, PackIndex, PackStack,
};

use crate::declaration::{Declaration, Source};
use crate::diagnostics::{Diagnostic, Diagnostics, Missing, Referrer};
use crate::order::entry_file_order;
use crate::registries::{Erased, Kind, Registries, Stage, Table};
use crate::registry::Names;
use crate::tag_file::{pack_id, resolve_tags};
use crate::tags::TagTable;

impl Registries {
    /// Loads `declarations` over `stack` from `files`, whose values move into the registries.
    pub fn load(
        declarations: Vec<Declaration>,
        stack: Arc<PackStack>,
        files: PackFiles,
    ) -> Result<(Registries, Diagnostics), Diagnostics> {
        let loading = Registries::new(Vec::new(), Arc::clone(&stack), stack);
        read(declarations, loading, files, None)
    }

    /// Loads these registries again over `stack` from `files`. The stages before
    /// [`Stage::Reloadable`] keep their entries and values, every tag is read again and keeps its
    /// number, and the reloadable stage is read again.
    pub fn reload(
        &self,
        stack: Arc<PackStack>,
        files: PackFiles,
    ) -> Result<(Registries, Diagnostics), Diagnostics> {
        let declarations = (self.tables().iter())
            .map(|table| Declaration {
                key: table.key.clone(),
                stage: table.stage,
                kind: table.kind.clone(),
                names: Vec::new(),
            })
            .collect();
        let loading = Registries::new(Vec::new(), Arc::clone(&self.world_stack), stack);
        read(declarations, loading, files, Some(self))
    }
}

#[derive(Default)]
pub(crate) struct Report {
    pub(crate) skipped: Vec<Diagnostic>,
    pub(crate) failed_tags: Vec<Diagnostic>,
    pub(crate) unresolved: BTreeMap<ResourceLocationBuf, Vec<Missing>>,
    pub(crate) errors: Vec<Diagnostic>,
}

impl Report {
    pub(crate) fn names(
        &mut self,
        registry: &ResourceLocationBuf,
        names: Vec<ResourceLocationBuf>,
        origins: Vec<PackIndex>,
    ) -> Option<Arc<Names>> {
        match Names::new(names, origins) {
            Ok(names) => Some(Arc::new(names)),
            Err(count) => {
                self.errors.push(Diagnostic::TooManyEntries {
                    registry: registry.clone(),
                    count,
                });
                None
            }
        }
    }

    pub(crate) fn tags(
        &mut self,
        registry: &ResourceLocationBuf,
        entries: usize,
        tags: Vec<(ResourceLocationBuf, Option<Vec<u16>>)>,
    ) -> Option<TagTable> {
        match TagTable::new(entries, tags) {
            Ok(tags) => Some(tags),
            Err(count) => {
                self.errors.push(Diagnostic::TooManyTags {
                    registry: registry.clone(),
                    count,
                });
                None
            }
        }
    }
}

pub(crate) struct Filled<'f> {
    pub(crate) names: Arc<Names>,
    pub(crate) tags: TagTable,
    pub(crate) kept: Option<Erased>,
    pub(crate) sources: Option<Vec<Source<'f>>>,
}

fn check(declarations: &[Declaration]) -> Result<(), Diagnostics> {
    let mut errors = Vec::new();
    let mut keys = HashSet::new();
    let mut paths = HashSet::new();
    for declaration in declarations {
        let registry = declaration.key.clone();
        let message = if !keys.insert(registry.clone()) {
            "the registry is declared twice"
        } else if !paths.insert(registry.path().to_owned()) {
            "another registry is declared with the same path"
        } else if declaration.names.iter().collect::<HashSet<_>>().len() != declaration.names.len()
        {
            "an entry name is listed twice"
        } else {
            continue;
        };
        errors.push(Diagnostic::Declaration {
            registry,
            message: message.to_owned(),
        });
    }
    if errors.is_empty() {
        return Ok(());
    }
    Err(Diagnostics::default().with_report(errors))
}

/// Loads `declarations` stage by stage into `registries`, which resolve the stage being loaded.
/// `fill` names one registry and gives the tags and the entry sources of it. A load from what a
/// server sent runs no validator and holds only its synced registries to be non-empty.
pub(crate) fn stages<'f>(
    mut declarations: Vec<Declaration>,
    mut registries: Registries,
    received: bool,
    mut diagnostics: Diagnostics,
    mut fill: impl FnMut(&mut Declaration, &mut Report) -> Option<Filled<'f>>,
) -> Result<(Registries, Diagnostics), Diagnostics> {
    check(&declarations)?;
    for stage in Stage::ALL {
        let mut report = Report::default();
        let mut decoding = Vec::new();
        for declaration in declarations.iter_mut() {
            if declaration.stage != stage {
                continue;
            }
            let Some(filled) = fill(declaration, &mut report) else {
                continue;
            };
            if let (Kind::Data(_), Some(sources)) = (&declaration.kind, filled.sources) {
                decoding.push((declaration.key.clone(), sources));
            }
            registries.insert(Table {
                key: declaration.key.clone(),
                stage,
                kind: declaration.kind.clone(),
                names: filled.names,
                tags: Arc::new(filled.tags),
                registry: filled.kept,
            });
        }
        diagnostics.extend(report.skipped);
        diagnostics.extend(report.failed_tags);
        let mut errors = report.errors;
        if errors.is_empty() {
            let mut unresolved = report.unresolved;
            let decoded = decode(&mut registries, decoding, &mut unresolved, &mut errors);
            errors.extend(
                (unresolved.into_iter())
                    .map(|(registry, missing)| Diagnostic::Unresolved { registry, missing }),
            );
            if errors.is_empty() {
                errors = validate(&registries, &decoded, received);
            }
        }
        if !errors.is_empty() {
            return Err(diagnostics.with_report(errors));
        }
    }
    let tables = (declarations.iter())
        .filter_map(|declaration| registries.table(declaration.key.as_location()).cloned())
        .collect();
    let (world_stack, reload_stack) = (registries.world_stack, registries.reload_stack);
    Ok((
        Registries::new(tables, world_stack, reload_stack),
        diagnostics,
    ))
}

/// A miss into a registry of the stage being loaded is reported under that registry; a miss into
/// an earlier, complete stage is an error of the entry that names it.
fn decode(
    registries: &mut Registries,
    decoding: Vec<(ResourceLocationBuf, Vec<Source<'_>>)>,
    unresolved: &mut BTreeMap<ResourceLocationBuf, Vec<Missing>>,
    errors: &mut Vec<Diagnostic>,
) -> Vec<ResourceLocationBuf> {
    let mut built = Vec::new();
    for (key, sources) in decoding {
        let Some(table) = registries.table(key.as_location()) else {
            continue;
        };
        let Kind::Data(codec) = &table.kind else {
            continue;
        };
        let files: Vec<Option<&str>> = sources.iter().map(Source::file).collect();
        let failures = match codec.decode(&table.names, sources, registries) {
            Ok(registry) => {
                built.push((key, registry));
                continue;
            }
            Err(failures) => failures,
        };
        for (at, error) in failures {
            let number = u16::try_from(at).ok();
            let Some(entry) = number.and_then(|number| table.names.name(number)) else {
                continue;
            };
            let missing = error.missing_ref().filter(|missing| {
                (registries.table(missing.registry.as_location()))
                    .is_some_and(|t| t.stage == table.stage)
            });
            if let Some(missing) = missing {
                unresolved
                    .entry(missing.registry.clone())
                    .or_default()
                    .push(Missing {
                        name: missing.name.clone(),
                        tag: missing.tag,
                        referrer: Referrer::Entry {
                            registry: key.clone(),
                            entry: entry.clone(),
                        },
                    });
                continue;
            }
            let pack = number.and_then(|number| table.names.origin(number));
            errors.push(Diagnostic::Entry {
                registry: key.clone(),
                entry: entry.clone(),
                pack: pack.map(|pack| pack_id(registries.stack(table.stage), pack)),
                file: files.get(at).copied().flatten().map(str::to_owned),
                field: error.path().map(str::to_owned),
                message: error.message().to_owned(),
            });
        }
    }
    built
        .into_iter()
        .filter_map(|(key, registry)| {
            registries.table_mut(key.as_location())?.registry = Some(registry);
            Some(key)
        })
        .collect()
}

fn validate(
    registries: &Registries,
    decoded: &[ResourceLocationBuf],
    received: bool,
) -> Vec<Diagnostic> {
    let mut errors = Vec::new();
    for key in decoded {
        let Some(table) = registries.table(key.as_location()) else {
            continue;
        };
        let (Kind::Data(codec), Some(registry)) = (&table.kind, &table.registry) else {
            continue;
        };
        if codec.non_empty() && (!received || codec.synced()) && table.names.len() == 0 {
            errors.push(Diagnostic::EmptyRegistry {
                registry: key.clone(),
            });
            continue;
        }
        if received {
            continue;
        }
        for (at, message) in codec.validate(registry, registries) {
            let number = u16::try_from(at).ok();
            let entry = number.and_then(|number| table.names.name(number));
            errors.push(match entry {
                Some(entry) => Diagnostic::Entry {
                    registry: key.clone(),
                    entry: entry.clone(),
                    pack: (number.and_then(|number| table.names.origin(number)))
                        .map(|pack| pack_id(registries.stack(table.stage), pack)),
                    file: None,
                    field: None,
                    message,
                },
                None => Diagnostic::Declaration {
                    registry: key.clone(),
                    message: format!("the validator names entry {at}, which is absent: {message}"),
                },
            });
        }
    }
    errors
}

#[derive(Default)]
struct Files<'f> {
    entries: BTreeMap<ResourceLocationBuf, PackFile<'f>>,
    tags: BTreeMap<ResourceLocationBuf, Vec<PackFile<'f>>>,
}

fn read(
    declarations: Vec<Declaration>,
    registries: Registries,
    mut files: PackFiles,
    previous: Option<&Registries>,
) -> Result<(Registries, Diagnostics), Diagnostics> {
    let stack = Arc::clone(&registries.reload_stack);
    let mut diagnostics = Diagnostics::default();
    let (visible, ignored) = stack.files(&mut files);
    diagnostics.extend(ignored.into_iter().map(Diagnostic::Pack));
    let mut routed = route(&declarations, &stack, visible, &mut diagnostics);
    stages(
        declarations,
        registries,
        false,
        diagnostics,
        |declaration, report| {
            let files = routed.remove(&declaration.key).unwrap_or_default();
            let previous =
                previous.and_then(|previous| previous.table(declaration.key.as_location()));
            let kept = previous.filter(|_| declaration.stage < Stage::Reloadable);
            let (names, kept, sources) = match kept {
                Some(table) => (Arc::clone(&table.names), table.registry.clone(), None),
                None => {
                    let mut entries: Vec<_> = files.entries.into_iter().collect();
                    entries.sort_by(|a, b| entry_file_order(&a.0, &b.0));
                    let (listed, files): (Vec<_>, Vec<_>) = entries.into_iter().unzip();
                    let mut names = std::mem::take(&mut declaration.names);
                    names.extend(listed);
                    let origins = files.iter().map(|file| file.pack).collect();
                    let sources = matches!(declaration.kind, Kind::Data(_))
                        .then(|| files.into_iter().map(source).collect());
                    let names = report.names(&declaration.key, names, origins)?;
                    (names, None, sources)
                }
            };
            let tags = resolve_tags(
                &declaration.key,
                &names,
                files.tags,
                &stack,
                previous.is_none() && declaration.stage != Stage::Static,
                previous.map(|table| &*table.tags),
                report,
            );
            Some(Filled {
                tags: report.tags(&declaration.key, names.len(), tags)?,
                names,
                kept,
                sources,
            })
        },
    )
}

fn source(file: PackFile<'_>) -> Source<'_> {
    match file.content {
        PackContent::Bytes(bytes) => Source::File {
            path: file.path,
            bytes,
        },
        PackContent::Value(value) => Source::Value(value),
    }
}

/// A static registry has no entry files, so a file below its directory is not an entry.
fn route<'f>(
    declarations: &[Declaration],
    stack: &PackStack,
    files: Vec<PackFile<'f>>,
    diagnostics: &mut Diagnostics,
) -> HashMap<ResourceLocationBuf, Files<'f>> {
    let directories: HashMap<&str, (&ResourceLocationBuf, bool)> = (declarations.iter())
        .map(|d| (d.key.path(), (&d.key, !matches!(d.kind, Kind::Fixed))))
        .collect();
    let mut routed: HashMap<ResourceLocationBuf, Files<'f>> = HashMap::new();
    for file in files {
        let (tag, inside) = match file.relative.strip_prefix("tags/") {
            Some(inside) => (true, inside),
            None => (false, file.relative),
        };
        let found = inside.match_indices('/').rev().find_map(|(at, _)| {
            let (directory, rest) = inside.split_at_checked(at)?;
            match directories.get(directory) {
                Some(&(key, has_entries)) if tag || has_entries => {
                    Some((key, rest.strip_prefix('/')?))
                }
                _ => None,
            }
        });
        let Some((key, rest)) = found else {
            continue;
        };
        let name = match rest.strip_suffix(".json") {
            Some(stem) => ResourceLocationBuf::new(file.namespace, stem).ok(),
            None if ResourceLocationBuf::new(file.namespace, file.relative).is_ok() => continue,
            None => None,
        };
        let Some(name) = name else {
            diagnostics.extend([Diagnostic::Pack(PackDiagnostic::InvalidFileName {
                pack: pack_id(stack, file.pack),
                zip: stack.get(file.pack).is_some_and(|pack| pack.is_zip()),
                namespace: file.namespace.to_owned(),
                path: file.relative.to_owned(),
            })]);
            continue;
        };
        let files = routed.entry(key.clone()).or_default();
        match tag {
            true => files.tags.entry(name).or_default().push(file),
            false => {
                files.entries.insert(name, file);
            }
        }
    }
    routed
}
