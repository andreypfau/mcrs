use crate::names::NameTable;
use crate::report::LoadReport;
use crate::set::{Column, RegistrySet, Values};
use mcrs_minecraft_core::resource_location::ResourceLocation;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::any::Any;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt;
use std::sync::Arc;

type Name = ResourceLocation<Arc<str>>;
type Failures = Vec<(usize, String)>;
type Validator = Box<dyn Fn(&(dyn Any + Send + Sync), &RegistrySet) -> Failures + Send + Sync>;
type Parse = fn(&[Option<&[u8]>]) -> Result<Column, Failures>;
type Encode = fn(&(dyn Any + Send + Sync), usize) -> Option<Result<String, serde_json::Error>>;
type Check<T> = fn(&[T], &RegistrySet) -> Failures;

pub struct PackFile {
    pub path: String,
    pub bytes: Option<Vec<u8>>,
}

pub struct Pack {
    pub name: String,
    pub files: Vec<PackFile>,
}

struct Codec {
    parse: Parse,
    encode: Encode,
    validators: Vec<Validator>,
}

struct Declaration {
    codec: Option<Codec>,
    non_empty: bool,
}

pub struct WorldRegistries {
    declared: BTreeMap<Name, Declaration>,
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

fn parse_column<T: DeserializeOwned + Send + Sync + 'static>(
    inputs: &[Option<&[u8]>],
) -> Result<Column, Failures> {
    let mut values = Vec::with_capacity(inputs.len());
    let mut failures = Vec::new();
    for (index, bytes) in inputs.iter().enumerate() {
        match bytes {
            None => failures.push((index, "the file has no content".to_owned())),
            Some(bytes) => match serde_json::from_slice::<T>(bytes) {
                Ok(value) => values.push(value),
                Err(error) => failures.push((index, error.to_string())),
            },
        }
    }
    if failures.is_empty() {
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
                        },
                    )
                })
                .collect(),
        }
    }

    pub fn from_datapack_report(json: &[u8]) -> Result<Self, serde_json::Error> {
        let report: DatapackReport = serde_json::from_slice(json)?;
        Ok(Self::new(
            report
                .registries
                .into_iter()
                .filter(|(_, flags)| flags.elements && !flags.stable)
                .map(|(registry, _)| registry),
        ))
    }

    pub fn declared(&self) -> impl Iterator<Item = &Name> {
        self.declared.keys()
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
            parse: parse_column::<T>,
            encode: encode_value::<T>,
            validators: Vec::new(),
        });
        self
    }

    pub fn non_empty(&mut self, registry: ResourceLocation<&'static str>) -> &mut Self {
        self.declaration(registry).non_empty = true;
        self
    }

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
        codec.validators.push(Box::new(move |column, set| {
            let values = column
                .downcast_ref::<Arc<[T]>>()
                .unwrap_or_else(|| panic!("registry {registry} does not parse the validated type"));
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
        let column = set.column_any(registry)?;
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

        let mut candidates: HashMap<&str, Vec<Candidate>> = HashMap::new();
        let mut tags: HashMap<Name, BTreeSet<Name>> = HashMap::new();
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
                        Ok(tag) => {
                            tags.entry(registry.clone()).or_default().insert(tag);
                        }
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
                            bytes: file.bytes.as_deref(),
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
                    // chisle: a pack may not redefine an entry of an earlier pack, where vanilla lets the later one win; stacking with override lifts this
                    report.entry(
                        registry,
                        &entry,
                        candidate.path,
                        format_args!(
                            "defined by both {}/{} and {}/{}",
                            packs[kept.pack as usize].name,
                            kept.path,
                            packs[candidate.pack as usize].name,
                            candidate.path
                        ),
                    );
                    continue;
                }
                previous = Some(candidate);
                match Name::read(&entry) {
                    Ok(name) => {
                        names.push(name);
                        origins.push(candidate.pack);
                        inputs.push(candidate.bytes);
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
            let tag_names = tags.remove(registry).unwrap_or_default();
            match NameTable::new(registry.clone(), names, tag_names) {
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

        let mut tables = Vec::new();
        for table in statics.tables() {
            let Some(mut added) = tags.remove(table.registry()) else {
                tables.push(Arc::clone(table));
                continue;
            };
            added.extend(table.tags().cloned());
            match NameTable::new(
                table.registry().clone(),
                table.names().iter().cloned(),
                added,
            ) {
                Ok(rebuilt) => tables.push(Arc::new(rebuilt)),
                Err(error) => {
                    report.whole_registry(table.registry(), &directory_of(table.registry()), error)
                }
            }
        }
        tables.extend(world.iter().map(|loaded| Arc::clone(&loaded.table)));
        let set = match RegistrySet::from_tables(tables) {
            Ok(set) => set,
            Err(error) => {
                report.invalid_report(error);
                return Err(report);
            }
        };

        let mut values = Values::default();
        set.scope(|| {
            for loaded in &world {
                let Some(codec) = loaded.codec else {
                    continue;
                };
                match (codec.parse)(&loaded.inputs) {
                    Ok(column) => {
                        for validator in &codec.validators {
                            for (index, message) in validator(&*column, &set) {
                                loaded.report(&mut report, index, message);
                            }
                        }
                        values.columns.insert(loaded.registry.clone(), column);
                    }
                    Err(failures) => {
                        for (index, message) in failures {
                            loaded.report(&mut report, index, message);
                        }
                    }
                }
            }
        });
        if !report.is_empty() {
            return Err(report);
        }

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
    bytes: Option<&'a [u8]>,
}

struct Loaded<'a> {
    registry: &'a Name,
    codec: Option<&'a Codec>,
    table: Arc<NameTable>,
    origins: Vec<u32>,
    inputs: Vec<Option<&'a [u8]>>,
    paths: Vec<&'a str>,
}

impl Loaded<'_> {
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
    use crate::entry_set::EntrySet;
    use crate::id::Id;
    use crate::registry::Registry;
    use mcrs_minecraft_core::registry_key::RegistryKey;
    use mcrs_minecraft_core::rl;

    #[derive(Debug, Deserialize, Serialize)]
    #[serde(deny_unknown_fields)]
    struct Variant {
        asset_id: String,
        next: Id<Variant>,
    }

    impl RegistryKey for Variant {
        const KEY: ResourceLocation<&'static str> = rl!("minecraft:test_variant");
    }

    struct Marker;

    impl RegistryKey for Marker {
        const KEY: ResourceLocation<&'static str> = rl!("minecraft:test_marker");
    }

    struct Fixed;

    impl RegistryKey for Fixed {
        const KEY: ResourceLocation<&'static str> = rl!("minecraft:test_static");
    }

    #[derive(Debug, Deserialize, Serialize)]
    #[serde(deny_unknown_fields)]
    struct Linked {
        marker: Option<Id<Marker>>,
        tag: Option<EntrySet<Marker>>,
    }

    impl RegistryKey for Linked {
        const KEY: ResourceLocation<&'static str> = rl!("minecraft:test_linked");
    }

    const VARIANT: &str = "minecraft:test_variant";
    const MARKER: &str = "minecraft:test_marker";
    const LINKED: &str = "minecraft:test_linked";

    fn registries() -> WorldRegistries {
        let mut registries = WorldRegistries::new(
            [Variant::KEY, Marker::KEY, Linked::KEY].map(ResourceLocation::<Arc<str>>::from),
        );
        registries
            .parse::<Variant>(Variant::KEY)
            .parse::<Linked>(Linked::KEY);
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
        }
    }

    fn load(packs: &[Pack]) -> Result<RegistrySet, LoadReport> {
        registries().load(&RegistrySet::new(), packs)
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

    fn marker_files(paths: &[&str]) -> Vec<Pack> {
        vec![pack(
            "vanilla",
            paths.iter().map(|path| names_only(path)).collect(),
        )]
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
        let set = registries.load(&RegistrySet::new(), &packs).unwrap();

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
                    names_only("minecraft/tags/test_marker/listed.json"),
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

        let statics = RegistrySet::new()
            .with(
                Registry::<Fixed>::new(
                    [ResourceLocation::parse("minecraft:one").unwrap()],
                    std::iter::empty(),
                )
                .unwrap(),
            )
            .unwrap();
        let tagged = marker_files(&["minecraft/tags/test_static/x.json"]);
        let set = registries().load(&statics, &tagged).unwrap();
        let table = set.table("minecraft:test_static").unwrap();
        assert!(table.has_tag("minecraft:x"));
        assert_eq!(table.number("minecraft:one"), Some(0));
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
        registries.non_empty(Marker::KEY);
        let text = registries
            .load(&RegistrySet::new(), &[])
            .err()
            .expect("an empty registry declared non-empty is refused")
            .to_string();
        assert_eq!(
            text,
            "registry load failed: 1 errors in 1 registries\n\
             minecraft:test_marker (minecraft/test_marker/): Registry must be non-empty: minecraft:test_marker"
        );
        let filled = marker_files(&["minecraft/test_marker/one.json"]);
        assert!(registries.load(&RegistrySet::new(), &filled).is_ok());
    }

    #[test]
    fn a_validator_reports_against_its_entry() {
        let mut registries = registries();
        registries.validate::<Variant>(Variant::KEY, |values, _| {
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
            .load(&RegistrySet::new(), &packs)
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
            if reversed {
                files.reverse();
            }
            vec![pack("vanilla", files)]
        }

        fn snapshot(set: &RegistrySet) -> (Vec<String>, Vec<String>, Vec<usize>) {
            let nexts = set
                .column::<Variant>(VARIANT)
                .unwrap()
                .iter()
                .map(|value| value.next.index())
                .collect();
            (names(set, VARIANT), names(set, MARKER), nexts)
        }

        let outcomes = [false, true].map(|reversed| {
            (
                snapshot(&load(&packs(reversed, false)).unwrap()),
                report(&packs(reversed, true)),
            )
        });
        assert_eq!(outcomes[0], outcomes[1]);
        assert_eq!(outcomes[0].0.0.len(), 3);
        assert_eq!(outcomes[0].1.lines().count(), 3, "{}", outcomes[0].1);
    }
}
