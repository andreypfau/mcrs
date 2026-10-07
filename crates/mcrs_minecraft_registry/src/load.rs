use crate::names::NameTable;
use crate::report::LoadReport;
use crate::set::{Column, RegistrySet};
use crate::tags::{TagProblem, TagRules, TagSource, build_tags};
use mcrs_minecraft_core::resource_location::ResourceLocation;
use mcrs_minecraft_nbt::tag::NbtTag;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::any::{Any, TypeId};
use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::sync::Arc;

type Name = ResourceLocation<Arc<str>>;
type Failures = Vec<(usize, String)>;
type Validator = Box<dyn Fn(&RegistrySet) -> Failures + Send + Sync>;
type Parse = fn(Vec<Input<'_>>) -> Result<Column, Failures>;
type EncodeResult = Result<String, serde_json::Error>;
type Encode = fn(&(dyn Any + Send + Sync), usize) -> Option<EncodeResult>;
type Check<T> = fn(&[T], &RegistrySet) -> Failures;
type Project = Box<
    dyn Fn(&RegistrySet, &str, usize) -> Option<Result<NbtTag, mcrs_minecraft_nbt::Error>>
        + Send
        + Sync,
>;

pub struct SyncedNbt(pub NbtTag);

pub struct PackFile {
    pub path: String,
    pub bytes: Option<Vec<u8>>,
}

pub struct Pack {
    pub name: String,
    pub files: Vec<PackFile>,
    pub built: Vec<Built>,
}

type BuiltValue = Box<dyn Any + Send + Sync>;
type Build = Box<dyn Fn(&RegistrySet) -> Result<Vec<BuiltValue>, Failures> + Send + Sync>;

pub struct Built {
    registry: ResourceLocation<&'static str>,
    names: Vec<Name>,
    files: Vec<String>,
    value_type: TypeId,
    value_name: &'static str,
    build: Build,
}

impl Built {
    pub fn new<T: Send + Sync + 'static>(
        registry: ResourceLocation<&'static str>,
        names: Vec<Name>,
        build: fn(&RegistrySet) -> Result<Vec<T>, Failures>,
    ) -> Self {
        let expected = names.len();
        Built {
            registry,
            files: names
                .iter()
                .map(|name| format!("{}.json", name.path()))
                .collect(),
            names,
            value_type: TypeId::of::<T>(),
            value_name: std::any::type_name::<T>(),
            build: Box::new(move |set| {
                let values = build(set)?;
                assert_eq!(
                    values.len(),
                    expected,
                    "the builder of {registry} returns one value per name"
                );
                Ok(values
                    .into_iter()
                    .map(|value| Box::new(value) as BuiltValue)
                    .collect())
            }),
        }
    }
}

struct Codec {
    value_type: TypeId,
    value_name: &'static str,
    parse: Parse,
    encode: Encode,
    validators: Vec<Validator>,
    split: Option<Split>,
}

struct Split {
    columns: Box<dyn Fn(&(dyn Any + Send + Sync)) -> Vec<(TypeId, Column)> + Send + Sync>,
    encode: Box<dyn Fn(&RegistrySet, &str, usize) -> Option<EncodeResult> + Send + Sync>,
}

struct Declaration {
    codec: Option<Codec>,
    non_empty: bool,
    sync: Option<Project>,
}

pub struct WorldRegistries {
    declared: BTreeMap<Name, Declaration>,
    sync_order: Vec<Name>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DatapackReport {
    #[allow(dead_code)]
    others: serde::de::IgnoredAny,
    registries: BTreeMap<Name, Flags>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Flags {
    elements: bool,
    stable: bool,
    #[allow(dead_code)]
    tags: bool,
}

#[derive(Clone, Copy)]
enum Source<'a> {
    File(Option<&'a [u8]>),
    Built { built: &'a Built, index: usize },
}

enum Input<'a> {
    File(Option<&'a [u8]>),
    Built(BuiltValue),
    Skipped,
}

fn parse_column<T: DeserializeOwned + Send + Sync + 'static>(
    inputs: Vec<Input<'_>>,
) -> Result<Column, Failures> {
    let mut values = Vec::with_capacity(inputs.len());
    let mut failures = Vec::new();
    let mut skipped = false;
    for (index, input) in inputs.into_iter().enumerate() {
        match input {
            Input::File(None) => failures.push((index, "the file has no content".to_owned())),
            Input::File(Some(bytes)) => match serde_json::from_slice::<T>(bytes) {
                Ok(value) => values.push(value),
                Err(error) => failures.push((index, error.to_string())),
            },
            Input::Built(value) => values.push(
                *value
                    .downcast::<T>()
                    .expect("a built value has the type its registry parses"),
            ),
            Input::Skipped => skipped = true,
        }
    }
    if failures.is_empty() && !skipped {
        Ok(Arc::new(Arc::<[T]>::from(values)))
    } else {
        Err(failures)
    }
}

fn encode_value<T: Serialize + 'static>(
    column: &(dyn Any + Send + Sync),
    index: usize,
) -> Option<Result<String, serde_json::Error>> {
    let values = column.downcast_ref::<Arc<[T]>>()?;
    Some(serde_json::to_string(values.get(index)?))
}

/// The parts a split registry stores, one column each.
pub trait Parts: Sized + Send + Sync + 'static {
    type Refs<'a>;

    fn columns(parts: Vec<Self>) -> Vec<(TypeId, Column)>;

    fn refs<'a>(set: &'a RegistrySet, registry: &str, index: usize) -> Option<Self::Refs<'a>>;
}

macro_rules! parts {
    ($(($part:ident, $column:ident, $value:ident)),+) => {
        impl<$($part: Send + Sync + 'static),+> Parts for ($($part,)+) {
            type Refs<'a> = ($(&'a $part,)+);

            fn columns(parts: Vec<Self>) -> Vec<(TypeId, Column)> {
                $(let mut $column = Vec::with_capacity(parts.len());)+
                for ($($value,)+) in parts {
                    $($column.push($value);)+
                }
                vec![$((TypeId::of::<$part>(), Arc::new(Arc::<[$part]>::from($column)) as Column)),+]
            }

            fn refs<'a>(set: &'a RegistrySet, registry: &str, index: usize) -> Option<Self::Refs<'a>> {
                Some(($(set.column::<$part>(registry)?.get(index)?,)+))
            }
        }
    };
}

parts!((A, a, va), (B, b, vb));
parts!((A, a, va), (B, b, vb), (C, c, vc));

impl WorldRegistries {
    pub fn new(declared: impl IntoIterator<Item = Name>) -> Self {
        WorldRegistries {
            declared: declared
                .into_iter()
                .map(|registry| {
                    (
                        registry,
                        Declaration {
                            codec: None,
                            non_empty: false,
                            sync: None,
                        },
                    )
                })
                .collect(),
            sync_order: Vec::new(),
        }
    }

    pub fn from_datapack_report(json: &[u8]) -> Result<Self, serde_json::Error> {
        Self::from_report_where(json, |flags| flags.elements && !flags.stable)
    }

    /// The registries a data pack reload reads: recipes, loot tables,
    /// predicates and the like, loaded over the world registries they name.
    pub fn reloadable_from_datapack_report(json: &[u8]) -> Result<Self, serde_json::Error> {
        Self::from_report_where(json, |flags| flags.elements && flags.stable)
    }

    fn from_report_where(
        json: &[u8],
        keep: impl Fn(&Flags) -> bool,
    ) -> Result<Self, serde_json::Error> {
        let report: DatapackReport = serde_json::from_slice(json)?;
        Ok(Self::new(
            report
                .registries
                .into_iter()
                .filter(|(_, flags)| keep(flags))
                .map(|(registry, _)| registry),
        ))
    }

    pub fn declared(&self) -> impl Iterator<Item = &Name> {
        self.declared.keys()
    }

    pub fn synced(&self) -> impl Iterator<Item = &Name> {
        self.sync_order.iter()
    }

    pub fn parses(&self, registry: &str) -> bool {
        self.declared
            .get(registry)
            .is_some_and(|declaration| declaration.codec.is_some())
    }

    fn declaration(&mut self, registry: ResourceLocation<&'static str>) -> &mut Declaration {
        self.declared
            .get_mut(registry.as_str())
            .unwrap_or_else(|| panic!("registry {registry} is not a declared world registry"))
    }

    pub fn parse<T>(&mut self, registry: ResourceLocation<&'static str>) -> &mut Self
    where
        T: DeserializeOwned + Serialize + Send + Sync + 'static,
    {
        self.declaration(registry).codec = Some(Codec {
            value_type: TypeId::of::<T>(),
            value_name: std::any::type_name::<T>(),
            parse: parse_column::<T>,
            encode: encode_value::<T>,
            validators: Vec::new(),
            split: None,
        });
        self
    }

    /// Stores what `parse::<T>` read as the columns of the parts `split`
    /// returns, in place of `T`. Validators read the parts; `join` rebuilds `T`
    /// from the parts to encode an entry.
    pub fn split<T, P>(
        &mut self,
        registry: ResourceLocation<&'static str>,
        split: fn(&T) -> P,
        join: for<'a> fn(P::Refs<'a>) -> T,
    ) -> &mut Self
    where
        T: Serialize + Send + Sync + 'static,
        P: Parts,
    {
        let codec = self
            .declaration(registry)
            .codec
            .as_mut()
            .unwrap_or_else(|| panic!("registry {registry} parses no values to split"));
        assert_eq!(
            codec.value_type,
            TypeId::of::<T>(),
            "registry {registry} does not parse the split type"
        );
        codec.split = Some(Split {
            columns: Box::new(move |column| {
                let values = column
                    .downcast_ref::<Arc<[T]>>()
                    .expect("a parsed column holds the parsed type");
                P::columns(values.iter().map(split).collect())
            }),
            encode: Box::new(move |set, registry, index| {
                Some(serde_json::to_string(&join(P::refs(set, registry, index)?)))
            }),
        });
        self
    }

    pub fn non_empty(&mut self, registry: ResourceLocation<&'static str>) -> &mut Self {
        self.declaration(registry).non_empty = true;
        self
    }

    pub fn sync_value<T, N>(
        &mut self,
        registry: ResourceLocation<&'static str>,
        project: fn(&T) -> N,
    ) -> &mut Self
    where
        T: Send + Sync + 'static,
        N: Serialize + 'static,
    {
        let codec = self.codec(registry, "sync");
        assert_eq!(
            codec.value_type,
            TypeId::of::<T>(),
            "registry {registry} does not parse the synced type"
        );
        assert!(
            codec.split.is_none(),
            "registry {registry} is split: sync its parts"
        );
        self.sync_with(
            registry,
            Box::new(move |set, registry, index| {
                let value = set.column::<T>(registry)?.get(index)?;
                Some(mcrs_minecraft_nbt::to_nbt_tag(&project(value)))
            }),
        )
    }

    pub fn sync_parts<P, N>(
        &mut self,
        registry: ResourceLocation<&'static str>,
        project: for<'a> fn(P::Refs<'a>) -> N,
    ) -> &mut Self
    where
        P: Parts,
        N: Serialize + 'static,
    {
        assert!(
            self.codec(registry, "sync").split.is_some(),
            "registry {registry} is not split: sync its value"
        );
        self.sync_with(
            registry,
            Box::new(move |set, registry, index| {
                let parts = P::refs(set, registry, index)?;
                Some(mcrs_minecraft_nbt::to_nbt_tag(&project(parts)))
            }),
        )
    }

    fn codec(&mut self, registry: ResourceLocation<&'static str>, to: &str) -> &Codec {
        self.declaration(registry)
            .codec
            .as_ref()
            .unwrap_or_else(|| panic!("registry {registry} parses no values to {to}"))
    }

    fn sync_with(
        &mut self,
        registry: ResourceLocation<&'static str>,
        project: Project,
    ) -> &mut Self {
        let declaration = self.declaration(registry);
        assert!(
            declaration.sync.is_none(),
            "registry {registry} is synced twice"
        );
        declaration.sync = Some(project);
        self.sync_order.push(registry.into());
        self
    }

    /// Runs `check` once every registry has parsed, against the complete set.
    /// `T` is a type the registry stores a column of: its value, or a part of
    /// it when the registry is split.
    pub fn validate<T>(
        &mut self,
        registry: ResourceLocation<&'static str>,
        check: Check<T>,
    ) -> &mut Self
    where
        T: Send + Sync + 'static,
    {
        let codec = self
            .declaration(registry)
            .codec
            .as_mut()
            .unwrap_or_else(|| panic!("registry {registry} parses no values to validate"));
        codec.validators.push(Box::new(move |set| {
            let values = set
                .column::<T>(registry.as_str())
                .unwrap_or_else(|| panic!("registry {registry} does not store the validated type"));
            check(values, set)
        }));
        self
    }

    pub fn encode(
        &self,
        set: &RegistrySet,
        registry: &str,
        index: usize,
    ) -> Option<Result<String, serde_json::Error>> {
        let codec = self.declared.get(registry)?.codec.as_ref()?;
        if let Some(split) = &codec.split {
            return set.scope(|| (split.encode)(set, registry, index));
        }
        let column = set.column_any(registry, codec.value_type)?;
        set.scope(|| (codec.encode)(&**column, index))
    }

    pub fn load(&self, statics: &RegistrySet, packs: &[Pack]) -> Result<RegistrySet, LoadReport> {
        let mut report = LoadReport::new();

        let mut entry_directories: HashMap<&str, &Name> = HashMap::new();
        let mut tag_directories: HashMap<&str, &Name> = HashMap::new();
        for table in statics.tables() {
            tag_directories.insert(table.registry().path(), table.registry());
        }
        for registry in self.declared.keys() {
            entry_directories.insert(registry.path(), registry);
            tag_directories.insert(registry.path(), registry);
        }

        let labels: Vec<String> = packs
            .iter()
            .map(|pack| format!("{} (built in)", pack.name))
            .collect();
        let mut candidates: HashMap<&str, Vec<Candidate>> = HashMap::new();
        let mut tag_files: HashMap<&Name, BTreeMap<Name, Vec<TagSource<'_>>>> = HashMap::new();
        for (pack, contents) in packs.iter().enumerate() {
            for file in &contents.files {
                let Some((namespace, rest)) = file
                    .path
                    .split_once('/')
                    .filter(|(namespace, _)| !namespace.is_empty())
                else {
                    continue;
                };
                if let Some(path) = rest.strip_prefix("tags/") {
                    let Some((registry, file_name)) = locate(&tag_directories, path) else {
                        continue;
                    };
                    let Some(stem) = stem_of(file_name) else {
                        continue;
                    };
                    match Name::read(&format!("{namespace}:{stem}")) {
                        Ok(tag) => match file.bytes.as_deref() {
                            Some(bytes) => tag_files
                                .entry(registry)
                                .or_default()
                                .entry(tag)
                                .or_default()
                                .push(TagSource {
                                    pack: &contents.name,
                                    path: &file.path,
                                    bytes,
                                }),
                            None => report.entry(
                                registry,
                                &format!("#{tag}"),
                                &file.path,
                                "the file has no content",
                            ),
                        },
                        Err(error) => {
                            report.entry(
                                registry,
                                &format!("#{namespace}:{stem}"),
                                &file.path,
                                error,
                            );
                        }
                    }
                } else if let Some((registry, file_name)) = locate(&entry_directories, rest)
                    && stem_of(file_name).is_some()
                {
                    candidates
                        .entry(registry.as_str())
                        .or_default()
                        .push(Candidate {
                            file_name,
                            namespace,
                            pack: pack as u32,
                            path: &file.path,
                            source: Source::File(file.bytes.as_deref()),
                        });
                }
            }
            for built in &contents.built {
                let registry = Name::from(built.registry);
                let directory = directory_of(&registry);
                let Some((declared, declaration)) = self.declared.get_key_value(registry.as_str())
                else {
                    report.whole_registry(
                        &registry,
                        &directory,
                        "built entries name a registry that is not a declared world registry",
                    );
                    continue;
                };
                let Some(codec) = &declaration.codec else {
                    report.whole_registry(
                        declared,
                        &directory,
                        "built entries name a registry that parses no values",
                    );
                    continue;
                };
                assert_eq!(
                    built.value_type, codec.value_type,
                    "built entries of {declared} are {} and the registry parses {}",
                    built.value_name, codec.value_name
                );
                for (index, name) in built.names.iter().enumerate() {
                    candidates
                        .entry(declared.as_str())
                        .or_default()
                        .push(Candidate {
                            file_name: &built.files[index],
                            namespace: name.namespace(),
                            pack: pack as u32,
                            path: &labels[pack],
                            source: Source::Built { built, index },
                        });
                }
            }
        }

        let mut world = Vec::with_capacity(self.declared.len());
        for (registry, declaration) in &self.declared {
            let directory = directory_of(registry);
            let mut found = candidates.remove(registry.as_str()).unwrap_or_default();
            found.sort_by(|a, b| {
                a.file_name
                    .cmp(b.file_name)
                    .then_with(|| a.namespace.cmp(b.namespace))
                    .then_with(|| a.pack.cmp(&b.pack))
                    .then_with(|| a.is_built().cmp(&b.is_built()))
            });

            let mut names = Vec::new();
            let mut origins = Vec::new();
            let mut inputs = Vec::new();
            let mut paths = Vec::new();
            let mut previous: Option<&Candidate> = None;
            for candidate in &found {
                let stem = stem_of(candidate.file_name).unwrap_or(candidate.file_name);
                let entry = format!("{}:{stem}", candidate.namespace);
                let repeated = previous.filter(|kept| {
                    kept.file_name == candidate.file_name && kept.namespace == candidate.namespace
                });
                if let Some(kept) = repeated {
                    if kept.pack == candidate.pack && !kept.is_built() && candidate.is_built() {
                        continue;
                    }
                    // chisle: a pack may not redefine an entry of an earlier pack, where vanilla lets the later one win; stacking with override lifts this
                    report.entry(
                        registry,
                        &entry,
                        candidate.path,
                        format_args!(
                            "defined by both {} and {}",
                            kept.origin(packs),
                            candidate.origin(packs)
                        ),
                    );
                    continue;
                }
                previous = Some(candidate);
                match Name::read(&entry) {
                    Ok(name) => {
                        names.push(name);
                        origins.push(candidate.pack);
                        inputs.push(candidate.source);
                        paths.push(candidate.path);
                    }
                    Err(error) => report.entry(registry, &entry, candidate.path, error),
                }
            }

            if declaration.non_empty && names.is_empty() {
                report.whole_registry(
                    registry,
                    &directory,
                    format_args!("Registry must be non-empty: {registry}"),
                );
            }
            if statics.table(registry.as_str()).is_some() {
                report.whole_registry(
                    registry,
                    &directory,
                    "the registry is declared as a world registry and is also static",
                );
                continue;
            }
            match NameTable::new(registry.clone(), names) {
                Ok(table) => world.push(Loaded {
                    registry,
                    codec: declaration.codec.as_ref(),
                    table: Arc::new(table),
                    origins,
                    inputs,
                    paths,
                }),
                Err(error) => report.whole_registry(registry, &directory, error),
            }
        }

        let tables = statics
            .tables()
            .cloned()
            .chain(world.iter().map(|loaded| Arc::clone(&loaded.table)));
        let set = match RegistrySet::from_tables(tables) {
            Ok(set) => set.with_types_of(statics),
            Err(error) => {
                report.invalid_report(error);
                return Err(report);
            }
        };

        let mut values = statics.values().clone();
        let built = statics
            .tables()
            .filter(|table| statics.tag_table(table.registry().as_str()).is_none())
            .map(|table| (table, TagRules::Static))
            .chain(world.iter().map(|loaded| (&loaded.table, TagRules::World)));
        for (table, rules) in built {
            let files: Vec<_> = tag_files
                .remove(table.registry())
                .unwrap_or_default()
                .into_iter()
                .collect();
            let (tags, problems) = build_tags(table, rules, &files, None);
            for problem in problems {
                if let TagProblem::Error {
                    registry,
                    tag,
                    file,
                    message,
                } = problem
                {
                    report.entry(&registry, &format!("#{tag}"), &file, message);
                }
            }
            values.tags.insert(table.registry().clone(), Arc::new(tags));
        }

        let set = set.with_values(values.clone());
        let mut every_column_parsed = true;
        set.scope(|| {
            for loaded in &world {
                let Some(codec) = loaded.codec else {
                    continue;
                };
                match (codec.parse)(loaded.inputs(&set, &mut report)) {
                    Ok(column) => {
                        let columns = match &codec.split {
                            Some(split) => (split.columns)(&*column),
                            None => vec![(codec.value_type, column)],
                        };
                        values
                            .columns
                            .insert(loaded.registry.clone(), columns.into_iter().collect());
                    }
                    Err(failures) => {
                        every_column_parsed = false;
                        for (index, message) in failures {
                            loaded.report(&mut report, index, message);
                        }
                    }
                }
            }
        });
        if !every_column_parsed {
            return Err(report);
        }

        let set = set.with_values(values.clone());
        set.scope(|| {
            for loaded in &world {
                let Some(codec) = loaded.codec else {
                    continue;
                };
                for validator in &codec.validators {
                    for (index, message) in validator(&set) {
                        loaded.report(&mut report, index, message);
                    }
                }
            }
        });
        if !report.is_empty() {
            return Err(report);
        }

        set.scope(|| {
            for registry in &self.sync_order {
                let project = self.declared[registry.as_str()]
                    .sync
                    .as_ref()
                    .expect("a registry in the sync order has a projection");
                let loaded = world
                    .iter()
                    .find(|loaded| loaded.registry == registry)
                    .expect("a synced registry is loaded");
                let mut column = Vec::with_capacity(loaded.table.len());
                for index in 0..loaded.table.len() {
                    match project(&set, registry.as_str(), index)
                        .expect("a loaded registry holds the columns its projection reads")
                    {
                        Ok(tag) => column.push(SyncedNbt(tag)),
                        Err(error) => loaded.report(&mut report, index, error),
                    }
                }
                values.columns.entry(registry.clone()).or_default().insert(
                    TypeId::of::<SyncedNbt>(),
                    Arc::new(Arc::<[SyncedNbt]>::from(column)),
                );
            }
        });
        if !report.is_empty() {
            return Err(report);
        }
        values.synced.extend(self.sync_order.iter().cloned());

        values.packs = packs.iter().map(|pack| pack.name.as_str().into()).collect();
        for loaded in world {
            values
                .origins
                .insert(loaded.registry.clone(), loaded.origins.into_boxed_slice());
        }
        Ok(set.with_values(values))
    }
}

struct Candidate<'a> {
    file_name: &'a str,
    namespace: &'a str,
    pack: u32,
    path: &'a str,
    source: Source<'a>,
}

impl Candidate<'_> {
    fn is_built(&self) -> bool {
        matches!(self.source, Source::Built { .. })
    }

    fn origin(&self, packs: &[Pack]) -> String {
        match self.source {
            Source::File(_) => format!("{}/{}", packs[self.pack as usize].name, self.path),
            Source::Built { .. } => self.path.to_owned(),
        }
    }
}

struct Loaded<'a> {
    registry: &'a Name,
    codec: Option<&'a Codec>,
    table: Arc<NameTable>,
    origins: Vec<u32>,
    inputs: Vec<Source<'a>>,
    paths: Vec<&'a str>,
}

impl<'a> Loaded<'a> {
    fn inputs(&self, set: &RegistrySet, report: &mut LoadReport) -> Vec<Input<'a>> {
        let mut outputs: HashMap<*const Built, Option<Vec<Option<BuiltValue>>>> = HashMap::new();
        for (source, path) in self.inputs.iter().zip(&self.paths) {
            let Source::Built { built, .. } = source else {
                continue;
            };
            outputs
                .entry(std::ptr::from_ref(*built))
                .or_insert_with(|| match (built.build)(set) {
                    Ok(values) => Some(values.into_iter().map(Some).collect()),
                    Err(failures) => {
                        for (index, message) in failures {
                            report.entry(self.registry, built.names[index].as_str(), path, message);
                        }
                        None
                    }
                });
        }
        self.inputs
            .iter()
            .map(|source| match source {
                Source::File(bytes) => Input::File(*bytes),
                Source::Built { built, index, .. } => {
                    match outputs
                        .get_mut(&std::ptr::from_ref(*built))
                        .expect("every built of this registry was built")
                    {
                        Some(values) => {
                            Input::Built(values[*index].take().expect("a built name takes one id"))
                        }
                        None => Input::Skipped,
                    }
                }
            })
            .collect()
    }

    fn report(&self, report: &mut LoadReport, index: usize, message: impl fmt::Display) {
        let name = self
            .table
            .name(index)
            .expect("a failure names an entry of its registry");
        report.entry(self.registry, name.as_str(), self.paths[index], message);
    }
}

fn locate<'s, 'p>(
    directories: &HashMap<&'s str, &'s Name>,
    path: &'p str,
) -> Option<(&'s Name, &'p str)> {
    path.rmatch_indices('/').find_map(|(slash, _)| {
        let registry = directories.get(&path[..slash])?;
        Some((*registry, &path[slash + 1..]))
    })
}

fn stem_of(file_name: &str) -> Option<&str> {
    file_name
        .strip_suffix(".json")
        .filter(|stem| !stem.is_empty())
}

fn directory_of(registry: &Name) -> String {
    format!("{}/{}/", registry.namespace(), registry.path())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::holder_set::HolderSet;
    use crate::id::Id;
    use crate::registry::Registry;
    use mcrs_minecraft_core::registry_key::RegistryKey;
    use mcrs_minecraft_core::rl;
    use mcrs_minecraft_core::tag_key::TagKey;
    use mcrs_minecraft_nbt::tag::NbtTag;

    #[derive(Debug, Deserialize, Serialize)]
    #[serde(deny_unknown_fields)]
    struct Variant {
        asset_id: String,
        next: Id<Variant>,
    }

    impl Variant {
        const KEY: RegistryKey<Variant> = RegistryKey::new(rl!("minecraft:test_variant"));
    }

    #[derive(Debug, Deserialize, Serialize)]
    #[serde(deny_unknown_fields)]
    struct Pointer {
        variant: Id<Variant>,
    }

    impl Pointer {
        const KEY: RegistryKey<Pointer> = RegistryKey::new(rl!("minecraft:test_pointer"));
    }

    struct Marker;

    impl Marker {
        const KEY: RegistryKey<Marker> = RegistryKey::new(rl!("minecraft:test_marker"));
    }

    struct Fixed;

    impl Fixed {
        const KEY: RegistryKey<Fixed> = RegistryKey::new(rl!("minecraft:test_static"));
    }

    #[derive(Debug, Deserialize, Serialize)]
    #[serde(deny_unknown_fields)]
    struct Linked {
        marker: Option<Id<Marker>>,
        tag: Option<HolderSet<Marker>>,
    }

    impl Linked {
        const KEY: RegistryKey<Linked> = RegistryKey::new(rl!("minecraft:test_linked"));
    }

    #[derive(Debug, Deserialize, Serialize)]
    #[serde(deny_unknown_fields)]
    struct FixedLinked {
        tag: HolderSet<Fixed>,
    }

    impl FixedLinked {
        const KEY: RegistryKey<FixedLinked> = RegistryKey::new(rl!("minecraft:test_fixed_linked"));
    }

    const EMPTY_TAG: &str = r#"{"values":[]}"#;

    fn typed() -> RegistrySet {
        RegistrySet::new()
            .with_types([
                Variant::KEY.binding(),
                Pointer::KEY.binding(),
                Marker::KEY.binding(),
                Linked::KEY.binding(),
                FixedLinked::KEY.binding(),
            ])
            .unwrap()
    }

    fn fixed_statics() -> RegistrySet {
        typed()
            .with(Registry::<Fixed>::new(Fixed::KEY, [name("minecraft:one")]).unwrap())
            .unwrap()
    }

    const VARIANT: &str = "minecraft:test_variant";
    const MARKER: &str = "minecraft:test_marker";
    const LINKED: &str = "minecraft:test_linked";

    fn registries() -> WorldRegistries {
        let mut registries = WorldRegistries::new(
            [
                Variant::KEY.location(),
                Marker::KEY.location(),
                Linked::KEY.location(),
            ]
            .map(ResourceLocation::<Arc<str>>::from),
        );
        registries
            .parse::<Variant>(Variant::KEY.location())
            .parse::<Linked>(Linked::KEY.location());
        registries
    }

    fn data(path: &str, json: &str) -> PackFile {
        PackFile {
            path: path.to_owned(),
            bytes: Some(json.as_bytes().to_vec()),
        }
    }

    fn names_only(path: &str) -> PackFile {
        PackFile {
            path: path.to_owned(),
            bytes: None,
        }
    }

    fn variant(path: &str, asset_id: &str, next: &str) -> PackFile {
        data(
            path,
            &format!(r#"{{"asset_id":"{asset_id}","next":"{next}"}}"#),
        )
    }

    fn pack(name: &str, files: Vec<PackFile>) -> Pack {
        Pack {
            name: name.to_owned(),
            files,
            built: Vec::new(),
        }
    }

    fn load(packs: &[Pack]) -> Result<RegistrySet, LoadReport> {
        registries().load(&typed(), packs)
    }

    fn report(packs: &[Pack]) -> String {
        load(packs).err().expect("the load is refused").to_string()
    }

    fn names(set: &RegistrySet, registry: &str) -> Vec<String> {
        set.table(registry)
            .unwrap_or_else(|| panic!("{registry} is present"))
            .names()
            .iter()
            .map(ToString::to_string)
            .collect()
    }

    fn name(text: &str) -> Name {
        Name::read(text).unwrap()
    }

    type Builder = fn(&RegistrySet) -> Result<Vec<Variant>, Failures>;

    fn built_variants(
        registry: ResourceLocation<&'static str>,
        names: &[&str],
        build: Builder,
    ) -> Built {
        Built::new(
            registry,
            names.iter().map(|text| name(text)).collect(),
            build,
        )
    }

    fn next_is_c(set: &RegistrySet) -> Result<Vec<Variant>, Failures> {
        let table = set.table(VARIANT).unwrap();
        let next = Id::from_number(table.number("test:c").unwrap());
        Ok(vec![Variant {
            asset_id: "built".to_owned(),
            next,
        }])
    }

    fn first_entry(_: &RegistrySet) -> Result<Vec<Variant>, Failures> {
        Ok(vec![Variant {
            asset_id: "built".to_owned(),
            next: Id::from_number(0),
        }])
    }

    fn nothing(_: &RegistrySet) -> Result<Vec<Variant>, Failures> {
        Ok(Vec::new())
    }

    fn refuses(_: &RegistrySet) -> Result<Vec<Variant>, Failures> {
        Err(vec![(0, "no good".to_owned())])
    }

    fn built_one(name: &str, build: Builder) -> Pack {
        built_pack(
            "builtin",
            vec![built_variants(Variant::KEY.location(), &[name], build)],
        )
    }

    fn built_pack(name: &str, built: Vec<Built>) -> Pack {
        Pack {
            name: name.to_owned(),
            files: Vec::new(),
            built,
        }
    }

    fn marker_files(paths: &[&str]) -> Vec<Pack> {
        vec![pack(
            "vanilla",
            paths.iter().map(|path| names_only(path)).collect(),
        )]
    }

    #[test]
    fn a_split_registry_stores_both_columns_and_encodes_the_file_again() {
        let file = r#"{"asset_id":"plain","next":"minecraft:plain"}"#;
        let packs = [pack(
            "vanilla",
            vec![data("minecraft/test_variant/plain.json", file)],
        )];
        let mut registries = registries();
        registries
            .validate::<String>(Variant::KEY.location(), |values, _| {
                assert_eq!(values[0], "plain");
                Vec::new()
            })
            .split::<Variant, (String, Id<Variant>)>(
                Variant::KEY.location(),
                |variant| (variant.asset_id.clone(), variant.next),
                |(asset_id, next)| Variant {
                    asset_id: asset_id.clone(),
                    next: *next,
                },
            );
        let set = registries.load(&typed(), &packs).unwrap();

        let registry = Variant::KEY.location().as_static_str();
        assert_eq!(set.column::<String>(registry).unwrap(), ["plain"]);
        assert_eq!(set.column::<Id<Variant>>(registry).unwrap()[0].index(), 0);
        assert!(set.column::<Variant>(registry).is_none());
        assert_eq!(registries.encode(&set, registry, 0).unwrap().unwrap(), file);
    }

    #[test]
    fn a_pack_loads_into_ids_and_values() {
        let packs = [
            pack(
                "vanilla",
                vec![
                    variant(
                        "minecraft/test_variant/plain.json",
                        "plain",
                        "minecraft:plain",
                    ),
                    names_only("minecraft/test_marker/two.json"),
                    names_only("minecraft/test_marker/one.json"),
                ],
            ),
            pack(
                "extra",
                vec![
                    variant("a/test_variant/zeta.json", "zeta", "b:alpha"),
                    variant("b/test_variant/alpha.json", "alpha", "a:zeta"),
                ],
            ),
        ];
        let registries = registries();
        let set = registries.load(&typed(), &packs).unwrap();

        assert_eq!(
            names(&set, VARIANT),
            ["b:alpha", "minecraft:plain", "a:zeta"]
        );
        let values = set.column::<Variant>(VARIANT).unwrap();
        let assets: Vec<&str> = values.iter().map(|value| value.asset_id.as_str()).collect();
        assert_eq!(assets, ["alpha", "plain", "zeta"]);
        let nexts: Vec<usize> = values.iter().map(|value| value.next.index()).collect();
        assert_eq!(nexts, [2, 1, 0]);

        assert_eq!(names(&set, MARKER), ["minecraft:one", "minecraft:two"]);
        assert!(set.column::<Variant>(MARKER).is_none());

        assert_eq!(set.pack_of(VARIANT, 0), Some("extra"));
        assert_eq!(set.pack_of(VARIANT, 1), Some("vanilla"));
        assert_eq!(set.pack_of(MARKER, 0), Some("vanilla"));
        assert_eq!(set.pack_of(VARIANT, 3), None);

        let written: Vec<String> = (0..3)
            .map(|index| registries.encode(&set, VARIANT, index).unwrap().unwrap())
            .collect();
        assert_eq!(
            written,
            [
                r#"{"asset_id":"alpha","next":"a:zeta"}"#,
                r#"{"asset_id":"plain","next":"minecraft:plain"}"#,
                r#"{"asset_id":"zeta","next":"b:alpha"}"#,
            ]
        );
    }

    #[test]
    fn entries_from_the_set_share_the_column() {
        let packs = [pack(
            "vanilla",
            vec![
                variant("minecraft/test_variant/a.json", "a", "minecraft:b"),
                variant("minecraft/test_variant/b.json", "b", "minecraft:a"),
            ],
        )];
        let set = load(&packs).unwrap();
        let first = set.entries::<Variant, Variant>().unwrap();
        let second = set.entries::<Variant, Variant>().unwrap();
        assert!(first.shares_with(&second));
        let column = set.column::<Variant>(VARIANT).unwrap();
        assert_eq!(first.as_slice().len(), column.len());
        for (entry, expected) in first.as_slice().iter().zip(column) {
            assert_eq!(entry.asset_id, expected.asset_id);
        }
        assert!(set.entries::<Variant, Marker>().is_none());
        assert!(set.entries::<Marker, Variant>().is_none());
    }

    #[test]
    fn a_broken_file_yields_a_report_and_no_set() {
        let packs = [pack(
            "vanilla",
            vec![data(
                "minecraft/test_variant/broken.json",
                r#"{"asset_id":"x","next":"minecraft:broken","bogus":1}"#,
            )],
        )];
        let text = report(&packs);
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 2, "{text}");
        assert_eq!(lines[0], "registry load failed: 1 errors in 1 registries");
        assert!(
            lines[1].starts_with(
                "minecraft:test_variant/minecraft:broken (minecraft/test_variant/broken.json): "
            ),
            "{text}"
        );
        assert!(lines[1].contains("unknown field `bogus`"), "{text}");
    }

    #[test]
    fn entries_sort_by_file_name_then_namespace() {
        let cases: [(&[&str], &[&str]); 2] = [
            (
                &[
                    "a/test_marker/zeta.json",
                    "b/test_marker/same.json",
                    "b/test_marker/alpha.json",
                    "a/test_marker/same.json",
                ],
                &["b:alpha", "a:same", "b:same", "a:zeta"],
            ),
            (
                &[
                    "minecraft/test_marker/foo/bar.json",
                    "minecraft/test_marker/foo.json",
                    "minecraft/test_marker/foo-bar.json",
                ],
                &["minecraft:foo-bar", "minecraft:foo", "minecraft:foo/bar"],
            ),
        ];
        for (paths, expected) in cases {
            let set = load(&marker_files(paths)).unwrap();
            assert_eq!(names(&set, MARKER), expected, "{paths:?}");
        }
    }

    #[test]
    fn an_empty_declared_registry_is_present_and_empty() {
        let set = load(&[]).unwrap();
        for registry in [VARIANT, MARKER, LINKED] {
            assert!(set.table(registry).unwrap().is_empty(), "{registry}");
        }
        assert!(set.column::<Variant>(VARIANT).unwrap().is_empty());
        assert!(set.column::<Linked>(LINKED).unwrap().is_empty());
        assert!(set.column::<Linked>(MARKER).is_none());
    }

    #[test]
    fn a_dangling_reference_names_the_registry_and_the_name() {
        let packs = [pack(
            "vanilla",
            vec![
                names_only("minecraft/test_marker/present.json"),
                data(
                    "minecraft/test_linked/a.json",
                    r#"{"marker":"minecraft:absent"}"#,
                ),
            ],
        )];
        let text = report(&packs);
        let line = text.lines().nth(1).unwrap();
        assert!(
            line.starts_with("minecraft:test_linked/minecraft:a (minecraft/test_linked/a.json): "),
            "{text}"
        );
        assert!(line.contains("minecraft:test_marker"), "{text}");
        assert!(line.contains("minecraft:absent"), "{text}");
    }

    #[test]
    fn a_tag_reference_is_checked_against_the_tag_files() {
        fn linked_to(tag: &str) -> Vec<Pack> {
            vec![pack(
                "vanilla",
                vec![
                    names_only("minecraft/test_marker/m.json"),
                    data("minecraft/tags/test_marker/listed.json", EMPTY_TAG),
                    data(
                        "minecraft/test_linked/l.json",
                        &format!(r##"{{"tag":"#{tag}"}}"##),
                    ),
                ],
            )]
        }
        assert!(load(&linked_to("minecraft:listed")).is_ok());
        let text = report(&linked_to("minecraft:unlisted"));
        assert!(text.contains("minecraft:unlisted"), "{text}");
        assert!(text.contains("minecraft:test_linked/minecraft:l"), "{text}");

        let tagged = vec![pack(
            "vanilla",
            vec![data("minecraft/tags/test_static/x.json", EMPTY_TAG)],
        )];
        let set = registries().load(&fixed_statics(), &tagged).unwrap();
        let table = set.table("minecraft:test_static").unwrap();
        assert!(
            set.tags::<Fixed>()
                .unwrap()
                .get(&TagKey::<Fixed, _>::from_location(name("minecraft:x")))
                .is_some()
        );
        assert_eq!(table.number("minecraft:one"), Some(0));
    }

    enum Expect {
        Loads {
            present: &'static [&'static str],
            absent: &'static [&'static str],
        },
        Fails(&'static [&'static str]),
    }

    #[test]
    fn tag_outcomes_on_the_load() {
        let cases: [(&str, Vec<PackFile>, Expect); 6] = [
            (
                "a world registry tag with a missing required element refuses the load",
                vec![
                    names_only("minecraft/test_marker/a.json"),
                    data(
                        "minecraft/tags/test_marker/t.json",
                        r#"{"values":["minecraft:a","minecraft:absent"]}"#,
                    ),
                ],
                Expect::Fails(&[
                    "minecraft:test_marker/#minecraft:t (minecraft/tags/test_marker/t.json): ",
                    "minecraft:absent",
                ]),
            ),
            (
                "a static registry tag with a missing required element is dropped",
                vec![
                    data(
                        "minecraft/tags/test_static/broken.json",
                        r#"{"values":["minecraft:absent"]}"#,
                    ),
                    data(
                        "minecraft/tags/test_static/good.json",
                        r#"{"values":["minecraft:one"]}"#,
                    ),
                ],
                Expect::Loads {
                    present: &["minecraft:good"],
                    absent: &["minecraft:broken"],
                },
            ),
            (
                "a value naming a dropped tag refuses the load",
                vec![
                    data(
                        "minecraft/tags/test_static/broken.json",
                        r#"{"values":["minecraft:absent"]}"#,
                    ),
                    data(
                        "minecraft/test_fixed_linked/v.json",
                        r##"{"tag":"#minecraft:broken"}"##,
                    ),
                ],
                Expect::Fails(&[
                    "minecraft:test_fixed_linked/minecraft:v (minecraft/test_fixed_linked/v.json): ",
                    "Missing tag: 'minecraft:broken'",
                ]),
            ),
            (
                "a value naming a tag no pack defines refuses the load",
                vec![data(
                    "minecraft/test_fixed_linked/v.json",
                    r##"{"tag":"#minecraft:never"}"##,
                )],
                Expect::Fails(&["Missing tag: 'minecraft:never'"]),
            ),
            (
                "a value naming a tag whose only file is malformed refuses the load",
                vec![
                    data("minecraft/tags/test_static/lone.json", "{"),
                    data(
                        "minecraft/test_fixed_linked/v.json",
                        r##"{"tag":"#minecraft:lone"}"##,
                    ),
                ],
                Expect::Fails(&[
                    "minecraft:test_fixed_linked/minecraft:v (minecraft/test_fixed_linked/v.json): ",
                    "Missing tag: 'minecraft:lone'",
                ]),
            ),
            (
                "a malformed tag file is skipped and the others still load",
                vec![
                    data("minecraft/tags/test_static/bad.json", "{"),
                    data(
                        "minecraft/tags/test_static/good.json",
                        r#"{"values":["minecraft:one"]}"#,
                    ),
                ],
                Expect::Loads {
                    present: &["minecraft:good"],
                    absent: &[],
                },
            ),
        ];
        let mut world = WorldRegistries::new(
            [Marker::KEY.location(), FixedLinked::KEY.location()]
                .map(ResourceLocation::<Arc<str>>::from),
        );
        world.parse::<FixedLinked>(FixedLinked::KEY.location());
        for (case, files, expect) in cases {
            let loaded = world.load(&fixed_statics(), &[pack("vanilla", files)]);
            match (loaded, expect) {
                (Ok(set), Expect::Loads { present, absent }) => {
                    let tags = set.tags::<Fixed>().expect("the static registry has tags");
                    let has = |tag: &str| {
                        tags.get(&TagKey::<Fixed, _>::from_location(name(tag)))
                            .is_some()
                    };
                    for tag in present {
                        assert!(has(tag), "{case}: {tag} is built");
                    }
                    for tag in absent {
                        assert!(!has(tag), "{case}: {tag} is not built");
                    }
                }
                (Err(report), Expect::Fails(needles)) => {
                    let text = report.to_string();
                    assert_eq!(text.lines().count(), 2, "{case}: {text}");
                    for needle in needles {
                        assert!(text.contains(needle), "{case}: {needle} in {text}");
                    }
                }
                (Ok(_), Expect::Fails(_)) => panic!("{case}: the load is refused"),
                (Err(report), Expect::Loads { .. }) => panic!("{case}: the load holds: {report}"),
            }
        }
    }

    #[test]
    fn the_report_is_sorted_by_registry_entry_and_file() {
        let packs = [
            pack(
                "first",
                vec![
                    data("minecraft/test_variant/z.json", "{"),
                    names_only("minecraft/test_marker/Upper.json"),
                ],
            ),
            pack(
                "second",
                vec![
                    data("minecraft/test_linked/b.json", r#"{"marker":1}"#),
                    data("minecraft/test_variant/a.json", "[]"),
                ],
            ),
        ];
        let text = report(&packs);
        let mut lines = text.lines();
        assert_eq!(
            lines.next(),
            Some("registry load failed: 4 errors in 3 registries")
        );
        let expected = [
            "minecraft:test_linked/minecraft:b (minecraft/test_linked/b.json): ",
            "minecraft:test_marker/minecraft:Upper (minecraft/test_marker/Upper.json): ",
            "minecraft:test_variant/minecraft:a (minecraft/test_variant/a.json): ",
            "minecraft:test_variant/minecraft:z (minecraft/test_variant/z.json): ",
        ];
        for prefix in expected {
            let line = lines.next().unwrap_or_default();
            assert!(line.starts_with(prefix), "{prefix}\n{text}");
        }
        assert_eq!(lines.next(), None, "{text}");
    }

    #[test]
    fn an_entry_two_packs_define_names_both_files() {
        let file = "minecraft/test_variant/dup.json";
        let packs = [
            pack("vanilla", vec![variant(file, "one", "minecraft:dup")]),
            pack("extra", vec![variant(file, "two", "minecraft:dup")]),
        ];
        let text = report(&packs);
        assert_eq!(text.lines().count(), 2, "{text}");
        let line = text.lines().nth(1).unwrap();
        assert!(
            line.starts_with(
                "minecraft:test_variant/minecraft:dup (minecraft/test_variant/dup.json): "
            ),
            "{text}"
        );
        assert!(
            line.contains("vanilla/minecraft/test_variant/dup.json"),
            "{text}"
        );
        assert!(
            line.contains("extra/minecraft/test_variant/dup.json"),
            "{text}"
        );
    }

    #[test]
    fn a_non_empty_registry_without_entries_is_reported() {
        let mut registries = registries();
        registries.non_empty(Marker::KEY.location());
        let text = registries
            .load(&typed(), &[])
            .err()
            .expect("an empty registry declared non-empty is refused")
            .to_string();
        assert_eq!(
            text,
            "registry load failed: 1 errors in 1 registries\n\
             minecraft:test_marker (minecraft/test_marker/): Registry must be non-empty: minecraft:test_marker"
        );
        let filled = marker_files(&["minecraft/test_marker/one.json"]);
        assert!(registries.load(&typed(), &filled).is_ok());
    }

    #[test]
    fn a_validator_reports_against_its_entry() {
        let mut registries = registries();
        registries.validate::<Variant>(Variant::KEY.location(), |values, _| {
            assert_eq!(values.len(), 3);
            vec![
                (1, "bad asset".to_owned()),
                (1, "second complaint".to_owned()),
            ]
        });
        let packs = [pack(
            "vanilla",
            ["a", "b", "c"]
                .map(|name| {
                    variant(
                        &format!("minecraft/test_variant/{name}.json"),
                        name,
                        "minecraft:a",
                    )
                })
                .into(),
        )];
        let text = registries
            .load(&typed(), &packs)
            .err()
            .expect("the validator refuses the load")
            .to_string();
        let prefix = "minecraft:test_variant/minecraft:b (minecraft/test_variant/b.json): ";
        assert_eq!(
            text,
            format!(
                "registry load failed: 2 errors in 1 registries\n{prefix}bad asset\n{prefix}second complaint"
            )
        );
    }

    fn pointer_registries() -> WorldRegistries {
        let mut registries = WorldRegistries::new(
            [Variant::KEY.location(), Pointer::KEY.location()]
                .map(ResourceLocation::<Arc<str>>::from),
        );
        registries
            .parse::<Variant>(Variant::KEY.location())
            .parse::<Pointer>(Pointer::KEY.location());
        registries
    }

    fn pointer(name: &str, variant: &str) -> PackFile {
        data(
            &format!("minecraft/test_pointer/{name}.json"),
            &format!(r#"{{"variant":"{variant}"}}"#),
        )
    }

    #[test]
    fn a_validator_reads_another_registrys_column() {
        let mut registries = pointer_registries();
        registries
            .validate::<Pointer>(Pointer::KEY.location(), |pointers, set| {
                let variants = set
                    .column::<Variant>(VARIANT)
                    .expect("the variant column is stored before any validator runs");
                pointers
                    .iter()
                    .enumerate()
                    .filter(|(_, pointer)| variants[pointer.variant.index()].asset_id == "rejected")
                    .map(|(index, _)| (index, "points at a rejected variant".to_owned()))
                    .collect()
            })
            .validate::<Variant>(Variant::KEY.location(), |variants, _| {
                variants
                    .iter()
                    .enumerate()
                    .filter(|(_, variant)| variant.asset_id == "rejected")
                    .map(|(index, _)| (index, "is rejected".to_owned()))
                    .collect()
            });
        let packs = [pack(
            "vanilla",
            vec![
                variant("minecraft/test_variant/fine.json", "fine", "minecraft:fine"),
                variant(
                    "minecraft/test_variant/no.json",
                    "rejected",
                    "minecraft:fine",
                ),
                pointer("to_fine", "minecraft:fine"),
                pointer("to_no", "minecraft:no"),
            ],
        )];
        let text = registries
            .load(&typed(), &packs)
            .err()
            .expect("the validators refuse the load")
            .to_string();
        assert_eq!(
            text,
            "registry load failed: 2 errors in 2 registries\n\
             minecraft:test_pointer/minecraft:to_no (minecraft/test_pointer/to_no.json): points at a rejected variant\n\
             minecraft:test_variant/minecraft:no (minecraft/test_variant/no.json): is rejected"
        );
    }

    #[test]
    fn a_parse_failure_runs_no_validator() {
        let mut registries = pointer_registries();
        registries.validate::<Pointer>(Pointer::KEY.location(), |_, _| {
            panic!("a validator ran although a column failed to parse")
        });
        let packs = [pack(
            "vanilla",
            vec![
                variant("minecraft/test_variant/fine.json", "fine", "minecraft:fine"),
                data("minecraft/test_variant/broken.json", "{"),
                pointer("to_fine", "minecraft:fine"),
            ],
        )];
        let text = registries
            .load(&typed(), &packs)
            .err()
            .expect("the parse failure refuses the load")
            .to_string();
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 2, "{text}");
        assert!(
            lines[1].starts_with(
                "minecraft:test_variant/minecraft:broken (minecraft/test_variant/broken.json): "
            ),
            "{text}"
        );
    }

    #[test]
    fn a_validator_of_an_empty_registry_reports_nothing() {
        static RUNS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let mut registries = pointer_registries();
        registries.validate::<Pointer>(Pointer::KEY.location(), |pointers, _| {
            assert!(pointers.is_empty());
            RUNS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            Vec::new()
        });
        let packs = [pack(
            "vanilla",
            vec![variant(
                "minecraft/test_variant/fine.json",
                "fine",
                "minecraft:fine",
            )],
        )];
        registries.load(&typed(), &packs).unwrap();
        assert_eq!(RUNS.load(std::sync::atomic::Ordering::Relaxed), 1);
    }

    #[test]
    fn the_load_does_not_depend_on_file_order() {
        fn packs(reversed: bool, broken: bool) -> Vec<Pack> {
            let mut files = vec![
                variant("minecraft/test_variant/plain.json", "plain", "a:zeta"),
                variant("a/test_variant/zeta.json", "zeta", "b:alpha"),
                variant("b/test_variant/alpha.json", "alpha", "minecraft:plain"),
                names_only("minecraft/test_marker/two.json"),
                names_only("minecraft/test_marker/one.json"),
            ];
            if broken {
                files.push(data("minecraft/test_variant/bad.json", "{"));
                files.push(data("minecraft/test_linked/bad.json", r#"{"marker":2}"#));
            }
            let mut packs = vec![
                pack("vanilla", files),
                built_pack(
                    "first",
                    vec![
                        built_variants(
                            Variant::KEY.location(),
                            &["minecraft:made", "a:made"],
                            two_entries,
                        ),
                        built_variants(Variant::KEY.location(), &["b:made"], first_entry),
                    ],
                ),
                built_one("z:made", first_entry),
            ];
            if reversed {
                packs.reverse();
                packs[1].built.reverse();
            }
            packs
        }

        fn two_entries(_: &RegistrySet) -> Result<Vec<Variant>, Failures> {
            Ok(["first", "second"]
                .map(|asset_id| Variant {
                    asset_id: asset_id.to_owned(),
                    next: Id::from_number(1),
                })
                .into())
        }

        type Snapshot = (Vec<String>, Vec<String>, Vec<String>, Vec<Option<String>>);

        fn snapshot(set: &RegistrySet) -> Snapshot {
            let values = set
                .column::<Variant>(VARIANT)
                .unwrap()
                .iter()
                .map(|value| format!("{}>{}", value.asset_id, value.next.index()))
                .collect();
            let packs = (0..names(set, VARIANT).len())
                .map(|id| set.pack_of(VARIANT, id).map(str::to_owned))
                .collect();
            (names(set, VARIANT), names(set, MARKER), values, packs)
        }

        let outcomes = [false, true].map(|reversed| {
            (
                snapshot(&load(&packs(reversed, false)).unwrap()),
                report(&packs(reversed, true)),
            )
        });
        assert_eq!(outcomes[0], outcomes[1]);
        assert_eq!(outcomes[0].0.0.len(), 7);
        assert_eq!(outcomes[0].1.lines().count(), 3, "{}", outcomes[0].1);
    }

    #[test]
    fn a_built_entry_takes_the_id_of_the_file_it_stands_for() {
        let packs = [
            pack(
                "files",
                vec![
                    variant("test/test_variant/c.json", "c", "test:a"),
                    variant("test/test_variant/a.json", "a", "test:c"),
                ],
            ),
            built_one("test:b", next_is_c),
        ];
        let set = load(&packs).unwrap();

        assert_eq!(names(&set, VARIANT), ["test:a", "test:b", "test:c"]);
        let values = set.column::<Variant>(VARIANT).unwrap();
        assert_eq!(values[1].asset_id, "built");
        assert_eq!(values[1].next.index(), 2);
        assert_eq!(set.pack_of(VARIANT, 0), Some("files"));
        assert_eq!(set.pack_of(VARIANT, 1), Some("builtin"));
    }

    #[test]
    fn a_file_in_the_same_pack_replaces_a_built_entry() {
        let same_pack = Pack {
            built: vec![built_variants(
                Variant::KEY.location(),
                &["test:b"],
                next_is_c,
            )],
            ..pack(
                "files",
                vec![
                    variant("test/test_variant/b.json", "file", "test:c"),
                    variant("test/test_variant/c.json", "c", "test:b"),
                ],
            )
        };
        let set = load(&[same_pack]).unwrap();
        assert_eq!(names(&set, VARIANT), ["test:b", "test:c"]);
        assert_eq!(set.column::<Variant>(VARIANT).unwrap()[0].asset_id, "file");
        assert_eq!(set.pack_of(VARIANT, 0), Some("files"));
    }

    #[test]
    fn a_built_entry_and_a_file_of_another_pack_are_refused() {
        let packs = [
            pack(
                "files",
                vec![
                    variant("test/test_variant/b.json", "file", "test:c"),
                    variant("test/test_variant/c.json", "c", "test:b"),
                ],
            ),
            built_one("test:b", next_is_c),
        ];
        assert_eq!(
            report(&packs),
            "registry load failed: 1 errors in 1 registries\n\
             minecraft:test_variant/test:b (builtin (built in)): \
             defined by both files/test/test_variant/b.json and builtin (built in)"
        );
    }

    #[test]
    fn a_failing_builder_reports_into_the_one_report() {
        let packs = [
            built_one("test:b", refuses),
            pack("files", vec![data("minecraft/test_variant/bad.json", "{")]),
        ];
        let text = report(&packs);
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 3, "{text}");
        assert_eq!(lines[0], "registry load failed: 2 errors in 1 registries");
        assert!(
            lines[1].starts_with(
                "minecraft:test_variant/minecraft:bad (minecraft/test_variant/bad.json): "
            ),
            "{text}"
        );
        assert_eq!(
            lines[2],
            "minecraft:test_variant/test:b (builtin (built in)): no good"
        );
    }

    #[test]
    fn a_built_entry_for_a_registry_without_values_is_refused() {
        let packs = [built_pack(
            "builtin",
            vec![built_variants(
                Marker::KEY.location(),
                &["test:b"],
                first_entry,
            )],
        )];
        assert_eq!(
            report(&packs),
            "registry load failed: 1 errors in 1 registries\n\
             minecraft:test_marker (minecraft/test_marker/): \
             built entries name a registry that parses no values"
        );
    }

    #[test]
    #[should_panic(expected = "built entries of minecraft:test_linked")]
    fn a_built_entry_of_the_wrong_type_is_a_programming_error() {
        let packs = [built_pack(
            "builtin",
            vec![built_variants(
                Linked::KEY.location(),
                &["test:b"],
                first_entry,
            )],
        )];
        let _ = load(&packs);
    }

    #[test]
    fn built_entries_satisfy_the_non_empty_rule() {
        let mut registries = registries();
        registries.non_empty(Variant::KEY.location());
        let set = registries
            .load(&typed(), &[built_one("test:b", first_entry)])
            .unwrap();
        assert_eq!(names(&set, VARIANT), ["test:b"]);
        assert!(registries.load(&typed(), &[]).is_err());
    }

    #[test]
    fn a_built_without_names_adds_nothing() {
        let packs = [built_pack(
            "builtin",
            vec![built_variants(Variant::KEY.location(), &[], nothing)],
        )];
        let set = load(&packs).unwrap();
        assert!(names(&set, VARIANT).is_empty());
        assert!(set.column::<Variant>(VARIANT).unwrap().is_empty());
    }

    #[test]
    fn loading_twice_gives_equal_sets() {
        let registries = registries();
        let packs = [
            pack(
                "files",
                vec![
                    variant("test/test_variant/a.json", "a", "test:c"),
                    variant("test/test_variant/c.json", "c", "test:a"),
                    names_only("minecraft/test_marker/one.json"),
                ],
            ),
            built_one("test:b", next_is_c),
        ];
        let [first, second] = [(); 2].map(|()| registries.load(&typed(), &packs).unwrap());
        for registry in [VARIANT, MARKER, LINKED] {
            assert_eq!(names(&first, registry), names(&second, registry));
            for id in 0..names(&first, registry).len() {
                assert_eq!(first.pack_of(registry, id), second.pack_of(registry, id));
                let encoded = |set: &RegistrySet| {
                    registries
                        .encode(set, registry, id)
                        .map(|text| text.unwrap())
                };
                assert_eq!(encoded(&first), encoded(&second));
            }
        }
        assert_eq!(names(&first, VARIANT).len(), 3);
    }

    #[test]
    fn a_synced_registry_is_encoded_once_at_load_in_declared_order() {
        let packs = [pack(
            "vanilla",
            vec![variant(
                "minecraft/test_variant/plain.json",
                "plain",
                "minecraft:plain",
            )],
        )];
        let mut registries = registries();
        registries
            .sync_value::<Variant, _>(Variant::KEY.location(), |variant| variant.asset_id.clone())
            .sync_value::<Linked, _>(Linked::KEY.location(), |_| 7_i32);
        let set = registries.load(&typed(), &packs).unwrap();

        let synced: Vec<(&str, Vec<&NbtTag>)> = set
            .synced()
            .map(|(table, column)| {
                (
                    table.registry().as_str(),
                    column.iter().map(|nbt| &nbt.0).collect(),
                )
            })
            .collect();
        assert_eq!(
            synced,
            [
                (VARIANT, vec![&NbtTag::String("plain".to_owned())]),
                (LINKED, vec![]),
            ]
        );
        assert_eq!(
            registries
                .synced()
                .map(|name| name.as_str())
                .collect::<Vec<_>>(),
            [VARIANT, LINKED]
        );
    }

    #[test]
    fn a_value_that_does_not_encode_fails_the_load() {
        let packs = [pack(
            "vanilla",
            vec![variant(
                "minecraft/test_variant/plain.json",
                "plain",
                "minecraft:plain",
            )],
        )];
        let mut registries = registries();
        registries.sync_value::<Variant, _>(Variant::KEY.location(), |_| 'x');
        let text = registries
            .load(&typed(), &packs)
            .err()
            .expect("the load is refused")
            .to_string();
        assert!(
            text.contains("minecraft:test_variant/minecraft:plain"),
            "{text}"
        );
        assert!(text.contains("char"), "{text}");
    }
}
