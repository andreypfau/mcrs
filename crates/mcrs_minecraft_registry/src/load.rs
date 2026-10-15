use crate::assembly::AssemblyError;
use crate::report::LoadReport;
use crate::set::{Column, RegistrySet};
use mcrs_minecraft_core::resource_location::ResourceLocation;
use mcrs_minecraft_nbt::tag::NbtTag;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::any::{Any, TypeId};
use std::collections::BTreeMap;
use std::sync::Arc;

pub(crate) type Name = ResourceLocation<Arc<str>>;
pub(crate) type Failures = Vec<(usize, String)>;
type EncodeResult = Result<String, serde_json::Error>;
type Encode = fn(&(dyn Any + Send + Sync), usize) -> Option<EncodeResult>;
type Project = Box<
    dyn Fn(&RegistrySet, &str, usize) -> Option<Result<NbtTag, mcrs_minecraft_nbt::Error>>
        + Send
        + Sync,
>;

pub struct SyncedNbt(pub NbtTag);

pub const PACKS_ROOT: &str = "mcrs/datapacks";
pub const VANILLA_PACK: &str = "vanilla";

struct Codec {
    value_type: TypeId,
    value_name: &'static str,
    encode: Encode,
    split: Option<Split>,
}

type SplitColumns = Box<dyn Fn(&(dyn Any + Send + Sync)) -> Vec<(TypeId, Column)> + Send + Sync>;
type SplitEncode = Box<dyn Fn(&RegistrySet, &str, usize) -> Option<EncodeResult> + Send + Sync>;
type ReceiveColumns =
    Box<dyn Fn(Vec<NbtTag>) -> Result<Vec<(TypeId, Column)>, Failures> + Send + Sync>;

struct Split {
    columns: SplitColumns,
    encode: SplitEncode,
}

pub(crate) struct Receive {
    pub(crate) columns: ReceiveColumns,
    reencode: Box<dyn Fn(NbtTag) -> Result<NbtTag, mcrs_minecraft_nbt::Error> + Send + Sync>,
}

pub(crate) struct Declaration {
    codec: Option<Codec>,
    pub(crate) non_empty: bool,
    sync: Option<Project>,
    pub(crate) receive: Option<Receive>,
}

pub struct WorldRegistries {
    pub(crate) declared: BTreeMap<Name, Declaration>,
    pub(crate) sync_order: Vec<Name>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DatapackReport {
    #[expect(
        dead_code,
        reason = "listed so deny_unknown_fields accepts the report; nothing reads it"
    )]
    others: serde::de::IgnoredAny,
    registries: BTreeMap<Name, Flags>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Flags {
    elements: bool,
    stable: bool,
    #[expect(
        dead_code,
        reason = "listed so deny_unknown_fields accepts the report; the load decides tags from the packs"
    )]
    tags: bool,
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

parts!((A, a, va));
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
                            receive: None,
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
        T: Serialize + Send + Sync + 'static,
    {
        self.declaration(registry).codec = Some(Codec {
            value_type: TypeId::of::<T>(),
            value_name: std::any::type_name::<T>(),
            encode: encode_value::<T>,
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

    /// Declares how a client reads what `sync_value` or `sync_parts` sends: the
    /// network type `N` is decoded from the entry's NBT and becomes the columns
    /// of `P`.
    pub fn receive<N, P>(
        &mut self,
        registry: ResourceLocation<&'static str>,
        receive: fn(N) -> P,
    ) -> &mut Self
    where
        N: DeserializeOwned + Serialize + 'static,
        P: Parts,
    {
        let declaration = self.declaration(registry);
        assert!(
            declaration.sync.is_some(),
            "registry {registry} is not synced: declare what it sends first"
        );
        assert!(
            declaration.receive.is_none(),
            "registry {registry} is received twice"
        );
        declaration.receive = Some(Receive {
            columns: Box::new(move |tags| {
                let mut parts = Vec::with_capacity(tags.len());
                let mut failures = Vec::new();
                for (index, tag) in tags.into_iter().enumerate() {
                    match mcrs_minecraft_nbt::from_tag::<N>(tag) {
                        Ok(network) => parts.push(receive(network)),
                        Err(error) => failures.push((index, error.to_string())),
                    }
                }
                if failures.is_empty() {
                    Ok(P::columns(parts))
                } else {
                    Err(failures)
                }
            }),
            reencode: Box::new(|tag| {
                mcrs_minecraft_nbt::to_nbt_tag(&mcrs_minecraft_nbt::from_tag::<N>(tag)?)
            }),
        });
        self
    }

    /// Decodes `tag` as the network type of `registry` and encodes it again, in
    /// the scope of `set`. A codec that is not symmetric shows as a different
    /// tag.
    pub fn reencode(
        &self,
        set: &RegistrySet,
        registry: &str,
        tag: NbtTag,
    ) -> Option<Result<NbtTag, mcrs_minecraft_nbt::Error>> {
        let receive = self.declared.get(registry)?.receive.as_ref()?;
        Some(set.scope(|| (receive.reencode)(tag)))
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

    fn synced_columns<F>(&self, set: &RegistrySet, mut fail: F) -> Vec<(&Name, Vec<SyncedNbt>)>
    where
        F: FnMut(&Name, Option<usize>, String),
    {
        set.scope(|| {
            let mut columns = Vec::with_capacity(self.sync_order.len());
            for registry in &self.sync_order {
                let Some(project) = self
                    .declared
                    .get(registry.as_str())
                    .and_then(|declaration| declaration.sync.as_ref())
                else {
                    fail(
                        registry,
                        None,
                        "the registry declares no projection".to_owned(),
                    );
                    continue;
                };
                let Some(table) = set.table(registry.as_str()) else {
                    fail(
                        registry,
                        None,
                        "the registry set holds no such registry".to_owned(),
                    );
                    continue;
                };
                let mut column = Vec::with_capacity(table.len());
                for index in 0..table.len() {
                    match project(set, registry.as_str(), index) {
                        Some(Ok(tag)) => column.push(SyncedNbt(tag)),
                        Some(Err(error)) => fail(registry, Some(index), error.to_string()),
                        None => {
                            fail(
                                registry,
                                None,
                                "the registry set holds no column the projection reads".to_owned(),
                            );
                            break;
                        }
                    }
                }
                columns.push((registry, column));
            }
            columns
        })
    }

    #[doc(hidden)]
    pub fn sync(&self, set: RegistrySet) -> Result<RegistrySet, LoadReport> {
        let mut report = LoadReport::new();
        let columns = self.synced_columns(&set, |registry, index, message| {
            let entry = index.and_then(|index| set.table(registry.as_str())?.name(index));
            report.unfiled(registry, entry.map(ResourceLocation::as_str), message);
        });
        if !report.is_empty() {
            return Err(report);
        }
        columns
            .into_iter()
            .try_fold(set.clone(), |set, (registry, column)| {
                set.with_synced(registry.as_str(), column.into())
            })
            .map_err(LoadReport::invalid)
    }

    /// An unsplit column shares `values`.
    #[doc(hidden)]
    pub fn attach<T: Send + Sync + 'static>(
        &self,
        set: RegistrySet,
        registry: &str,
        values: Arc<[T]>,
    ) -> Result<RegistrySet, AssemblyError> {
        let codec = self
            .declared
            .get(registry)
            .and_then(|declaration| declaration.codec.as_ref())
            .ok_or_else(|| AssemblyError::NotParsed {
                registry: registry.to_owned(),
            })?;
        if codec.value_type != TypeId::of::<T>() {
            return Err(AssemblyError::WrongValueType {
                registry: registry.to_owned(),
                parses: codec.value_name,
                given: std::any::type_name::<T>(),
            });
        }
        match &codec.split {
            Some(split) => {
                let len = values.len();
                set.with_columns(registry, len, (split.columns)(&values))
            }
            None => set.with_column(registry, values),
        }
    }
}

pub(crate) fn directory_of(registry: &Name) -> String {
    format!("{}/{}/", registry.namespace(), registry.path())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::holder_set::HolderSet;
    use crate::id::Id;
    use crate::names::NameTable;
    use crate::tags::TagTable;
    use mcrs_minecraft_core::registry_key::RegistryKey;
    use mcrs_minecraft_core::rl;
    use serde::Deserialize;
    use serde::de::DeserializeOwned;

    #[derive(Debug, Deserialize, Serialize)]
    #[serde(deny_unknown_fields)]
    struct Variant {
        asset_id: String,
        next: Id<Variant>,
    }

    impl Variant {
        const KEY: RegistryKey<Variant> = RegistryKey::new(rl!("minecraft:test_variant"));
    }

    struct Marker;

    impl Marker {
        const KEY: RegistryKey<Marker> = RegistryKey::new(rl!("minecraft:test_marker"));
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

    const VARIANT: &str = "minecraft:test_variant";
    const MARKER: &str = "minecraft:test_marker";
    const LINKED: &str = "minecraft:test_linked";

    fn name(text: &str) -> Name {
        Name::read(text).unwrap()
    }

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

    fn table(registry: &str, names: &[&str]) -> Arc<NameTable> {
        Arc::new(NameTable::new(name(registry), names.iter().map(|text| name(text))).unwrap())
    }

    const MARKERS: [&str; 2] = ["minecraft:one", "minecraft:two"];

    fn tables(variants: &[&str], linked: &[&str]) -> RegistrySet {
        let markers = table(MARKER, &MARKERS);
        let tags = [
            (name("minecraft:first"), vec![1_u16, 0]),
            (name("minecraft:second"), vec![0]),
        ];
        RegistrySet::new()
            .with_types([
                Variant::KEY.binding(),
                Marker::KEY.binding(),
                Linked::KEY.binding(),
            ])
            .unwrap()
            .with_tables(vec![
                table(VARIANT, variants),
                Arc::clone(&markers),
                table(LINKED, linked),
            ])
            .unwrap()
            .with_tags(Arc::new(TagTable::numbered(&markers, tags).unwrap()))
    }

    fn decode<T: DeserializeOwned>(set: &RegistrySet, files: &[&str]) -> Arc<[T]> {
        set.scope(|| {
            files
                .iter()
                .map(|json| serde_json::from_str(json).unwrap())
                .collect()
        })
    }

    const PLAIN: &str = r#"{"asset_id":"plain","next":"minecraft:plain"}"#;

    fn with_plain(registries: &WorldRegistries) -> RegistrySet {
        let set = tables(&["minecraft:plain"], &[]);
        let variants = decode::<Variant>(&set, &[PLAIN]);
        registries.attach(set, VARIANT, variants).unwrap()
    }

    #[test]
    fn a_split_registry_stores_both_columns_and_encodes_the_file_again() {
        let mut registries = registries();
        registries.split::<Variant, (String, Id<Variant>)>(
            Variant::KEY.location(),
            |variant| (variant.asset_id.clone(), variant.next),
            |(asset_id, next)| Variant {
                asset_id: asset_id.clone(),
                next: *next,
            },
        );
        let set = with_plain(&registries);

        assert_eq!(set.column::<String>(VARIANT).unwrap(), ["plain"]);
        assert_eq!(set.column::<Id<Variant>>(VARIANT).unwrap()[0].index(), 0);
        assert!(set.column::<Variant>(VARIANT).is_none());
        assert_eq!(registries.encode(&set, VARIANT, 0).unwrap().unwrap(), PLAIN);
    }

    #[test]
    fn a_synced_registry_is_encoded_in_declared_order() {
        let mut registries = registries();
        registries
            .sync_value::<Variant, _>(Variant::KEY.location(), |variant| variant.asset_id.clone())
            .sync_value::<Linked, _>(Linked::KEY.location(), |_| 7_i32);
        let set = registries.sync(with_plain(&registries)).unwrap();

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
    fn a_value_that_does_not_encode_fails_the_sync() {
        let mut registries = registries();
        registries.sync_value::<Variant, _>(Variant::KEY.location(), |_| 'x');
        let text = registries
            .sync(with_plain(&registries))
            .err()
            .expect("the sync is refused")
            .to_string();
        assert!(
            text.contains("minecraft:test_variant/minecraft:plain"),
            "{text}"
        );
        assert!(text.contains("char"), "{text}");
    }

    const LINKED_FILES: [&str; 2] = [
        r##"{"marker":"minecraft:two","tag":"#minecraft:second"}"##,
        r#"{"tag":["minecraft:one","minecraft:two"]}"#,
    ];

    #[test]
    fn an_unsplit_column_shares_the_values_it_was_given() {
        let skeleton = tables(&[], &["minecraft:near", "minecraft:list"]);
        let values = decode::<Linked>(&skeleton, &LINKED_FILES);
        let registries = registries();

        type Attach = fn(&WorldRegistries, RegistrySet, Arc<[Linked]>) -> RegistrySet;
        let cases: [(&str, Attach); 2] = [
            ("with_column", |_, set, values| {
                set.with_column(LINKED, values).unwrap()
            }),
            ("attach", |registries, set, values| {
                registries.attach(set, LINKED, values).unwrap()
            }),
        ];
        for (label, attach) in cases {
            let set = attach(&registries, skeleton.clone(), Arc::clone(&values));
            let column = set.column::<Linked>(LINKED).unwrap();
            assert!(std::ptr::eq(column, &*values), "{label}");
            let entries = set.entries::<Linked, Linked>().unwrap();
            assert!(std::ptr::eq(entries.as_slice(), &*values), "{label}");
        }
    }

    #[test]
    fn assembly_refuses_parts_that_disagree_with_the_tables() {
        let mut registries = registries();
        registries.split::<Variant, (String, Id<Variant>)>(
            Variant::KEY.location(),
            |variant| (variant.asset_id.clone(), variant.next),
            |(asset_id, next)| Variant {
                asset_id: asset_id.clone(),
                next: *next,
            },
        );
        let set = tables(&["minecraft:plain", "minecraft:other"], &[]);
        let markers = Arc::clone(set.table(MARKER).unwrap());
        let synced: Arc<[SyncedNbt]> = [0, 1].map(|_| SyncedNbt(NbtTag::Byte(0))).into();
        let tag = |member: u16| (name("minecraft:t"), [member]);

        let cases: [(&str, Result<(), AssemblyError>, &str); 8] = [
            (
                "a column of the wrong length",
                set.clone().with_column(MARKER, Arc::from([1_u8])).map(drop),
                "has 2 entries but 1 values",
            ),
            (
                "a column of an absent registry",
                set.clone()
                    .with_column("minecraft:test_absent", Arc::from([1_u8]))
                    .map(drop),
                "holds no registry minecraft:test_absent",
            ),
            (
                "a tag member past the end",
                TagTable::numbered(&markers, [tag(2)]).map(drop),
                "holds the id 2",
            ),
            (
                "a tag listed twice",
                TagTable::numbered(&markers, [tag(0), tag(1)]).map(drop),
                "lists the tag #minecraft:t twice",
            ),
            (
                "an origin past the pack list",
                set.clone()
                    .with_origins(
                        vec!["vanilla".to_owned()],
                        vec![(MARKER.to_owned(), Box::from([0_u32, 1]))],
                    )
                    .map(drop),
                "comes from pack 1",
            ),
            (
                "values of another type",
                registries
                    .attach(set.clone(), VARIANT, Arc::from([1_u8]))
                    .map(drop),
                "was given values of u8",
            ),
            (
                "values for a registry that declares no type",
                registries
                    .attach(set.clone(), MARKER, Arc::from([1_u8]))
                    .map(drop),
                "declares no value type",
            ),
            (
                "a registry synced twice",
                set.clone()
                    .with_synced(MARKER, Arc::clone(&synced))
                    .and_then(|set| set.with_synced(MARKER, synced))
                    .map(drop),
                "synced twice",
            ),
        ];
        for (label, attempt, expected) in cases {
            let message = attempt.expect_err(label).to_string();
            assert!(message.contains(expected), "{label}: {message}");
        }
    }
}
