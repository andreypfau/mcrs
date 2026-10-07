use crate::load::{Name, VANILLA_PACK, WorldRegistries, directory_of};
use crate::names::NameTable;
use crate::report::LoadReport;
use crate::set::RegistrySet;
use crate::tags::assemble;
use mcrs_minecraft_nbt::serializer::WriteAdaptor;
use mcrs_minecraft_nbt::tag::NbtTag;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

const REGISTRY_DATA: &str = "registry data";
const TAG_DATA: &str = "tags";
const TAG_CAPACITY: usize = u16::MAX as usize + 1;

pub struct NetworkEntry {
    pub name: Name,
    pub data: Option<NbtTag>,
}

pub struct NetworkRegistry {
    pub registry: Name,
    pub entries: Vec<NetworkEntry>,
}

pub struct NetworkTags {
    pub registry: Name,
    pub tags: Vec<(Name, Vec<i32>)>,
}

/// The network form of the entries the local vanilla pack holds, kept for the
/// whole session to fill the entries a server sends without data.
#[derive(Default)]
pub struct KnownPackEntries {
    registries: HashMap<Name, HashMap<Name, NbtTag>>,
}

impl KnownPackEntries {
    pub fn from_set(set: &RegistrySet) -> Self {
        let registries = set
            .synced()
            .map(|(table, column)| {
                let registry = table.registry();
                let entries = table
                    .names()
                    .iter()
                    .zip(column)
                    .enumerate()
                    .filter(|(id, _)| set.pack_of(registry.as_str(), *id) == Some(VANILLA_PACK))
                    .map(|(_, (name, network))| (name.clone(), network.0.clone()))
                    .collect();
                (registry.clone(), entries)
            })
            .collect();
        KnownPackEntries { registries }
    }

    pub fn get(&self, registry: &str, entry: &str) -> Option<&NbtTag> {
        self.registries.get(registry)?.get(entry)
    }

    pub fn len(&self) -> usize {
        self.registries.values().map(HashMap::len).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The bytes the tags take on the wire, nameless root included.
    pub fn encoded_len(&self) -> usize {
        self.registries
            .values()
            .flat_map(HashMap::values)
            .map(|tag| {
                let mut bytes = Vec::new();
                tag.serialize(&mut WriteAdaptor::new(&mut bytes))
                    .expect("a tag read from the wire writes");
                bytes.len()
            })
            .sum()
    }
}

struct Received<'a> {
    registry: &'a Name,
    declaration: &'a crate::load::Declaration,
    table: Arc<NameTable>,
    data: Vec<Option<NbtTag>>,
}

impl WorldRegistries {
    /// Builds the registry set a client holds from what a server sent: the
    /// registries in the order they arrived, their entries in the order they
    /// were listed, then the tags. An entry without data is read from `known`.
    /// Any entry that cannot be read refuses the build.
    pub fn from_network(
        &self,
        statics: &RegistrySet,
        registries: Vec<NetworkRegistry>,
        tags: &[NetworkTags],
        known: Option<&KnownPackEntries>,
    ) -> Result<RegistrySet, LoadReport> {
        let mut report = LoadReport::new();
        let mut received: Vec<Received> = Vec::with_capacity(registries.len());
        let mut sent = HashSet::new();
        for network in registries {
            let NetworkRegistry { registry, entries } = network;
            let directory = directory_of(&registry);
            let Some((declared, declaration)) = self
                .declared
                .get_key_value(registry.as_str())
                .filter(|(_, declaration)| declaration.receive.is_some())
            else {
                report.whole_registry(
                    &registry,
                    &directory,
                    "the client does not sync this registry",
                );
                continue;
            };
            if !sent.insert(declared) {
                report.whole_registry(declared, &directory, "the registry is sent twice");
                continue;
            }
            if statics.table(registry.as_str()).is_some() {
                report.whole_registry(declared, &directory, "the registry is also static");
                continue;
            }
            if let Some((table, data)) =
                names_of(declared, declaration.non_empty, entries, &mut report)
            {
                received.push(Received {
                    registry: declared,
                    declaration,
                    table,
                    data,
                });
            }
        }
        for registry in &self.sync_order {
            let declaration = &self.declared[registry.as_str()];
            if declaration.receive.is_none() || sent.contains(registry) {
                continue;
            }
            if let Some((table, data)) =
                names_of(registry, declaration.non_empty, Vec::new(), &mut report)
            {
                received.push(Received {
                    registry,
                    declaration,
                    table,
                    data,
                });
            }
        }
        if !report.is_empty() {
            return Err(report);
        }

        let tables = statics
            .tables()
            .cloned()
            .chain(received.iter().map(|received| Arc::clone(&received.table)));
        let base = match RegistrySet::from_tables(tables) {
            Ok(set) => set.with_types_of(statics),
            Err(error) => {
                report.invalid_report(error);
                return Err(report);
            }
        };
        let mut values = statics.values().clone();
        for received in &received {
            values.tags.insert(
                received.registry.clone(),
                Arc::new(assemble(
                    &received.table,
                    std::iter::empty::<(Name, Box<[u16]>)>(),
                )),
            );
        }
        let set = base.with_values(values).tagged(tags, &mut report);

        let mut values = set.values().clone();
        set.scope(|| {
            for received in received {
                let Received {
                    registry,
                    declaration,
                    table,
                    data,
                } = received;
                let receive = declaration
                    .receive
                    .as_ref()
                    .expect("a received registry declares how it is read");
                let mut decodable = Vec::with_capacity(data.len());
                for (id, data) in data.into_iter().enumerate() {
                    let name = table.name(id).expect("an entry has a name");
                    let tag = match data {
                        Some(tag) => tag,
                        None => match known
                            .and_then(|known| known.get(registry.as_str(), name.as_str()))
                        {
                            Some(tag) => tag.clone(),
                            None => {
                                report.entry(
                                    registry,
                                    name.as_str(),
                                    REGISTRY_DATA,
                                    "the entry carries no data and no known pack holds it",
                                );
                                continue;
                            }
                        },
                    };
                    decodable.push(tag);
                }
                if decodable.len() != table.len() {
                    continue;
                }
                match (receive.columns)(decodable) {
                    Ok(columns) => {
                        values
                            .columns
                            .insert(registry.clone(), columns.into_iter().collect());
                    }
                    Err(failures) => {
                        for (id, message) in failures {
                            let name = table.name(id).expect("a failure names an entry");
                            report.entry(registry, name.as_str(), REGISTRY_DATA, message);
                        }
                    }
                }
            }
        });
        if !report.is_empty() {
            return Err(report);
        }
        Ok(set.with_values(values))
    }
}

fn names_of(
    registry: &Name,
    non_empty: bool,
    entries: Vec<NetworkEntry>,
    report: &mut LoadReport,
) -> Option<(Arc<NameTable>, Vec<Option<NbtTag>>)> {
    let directory = directory_of(registry);
    if non_empty && entries.is_empty() {
        report.whole_registry(
            registry,
            &directory,
            format_args!("Registry must be non-empty: {registry}"),
        );
        return None;
    }
    let mut seen = HashSet::with_capacity(entries.len());
    let mut names = Vec::with_capacity(entries.len());
    let mut data = Vec::with_capacity(entries.len());
    let mut repeated = false;
    for entry in entries {
        if seen.insert(entry.name.clone()) {
            names.push(entry.name);
            data.push(entry.data);
        } else {
            repeated = true;
            report.entry(
                registry,
                entry.name.as_str(),
                REGISTRY_DATA,
                "the entry is sent twice",
            );
        }
    }
    if repeated {
        return None;
    }
    match NameTable::new(registry.clone(), names) {
        Ok(table) => Some((Arc::new(table), data)),
        Err(error) => {
            report.whole_registry(registry, &directory, error);
            None
        }
    }
}

impl RegistrySet {
    /// Replaces the tags of the registries `tags` names, as the game does for
    /// the tags it receives in play. The other registries keep their tags.
    pub fn with_network_tags(&self, tags: &[NetworkTags]) -> Result<RegistrySet, LoadReport> {
        let mut report = LoadReport::new();
        let set = self.tagged(tags, &mut report);
        if report.is_empty() {
            Ok(set)
        } else {
            Err(report)
        }
    }

    fn tagged(&self, tags: &[NetworkTags], report: &mut LoadReport) -> RegistrySet {
        let mut values = self.values().clone();
        let mut named = HashSet::new();
        for update in tags {
            let registry = &update.registry;
            let directory = directory_of(registry);
            let Some(names) = self.table(registry.as_str()) else {
                report.whole_registry(registry, &directory, "the set has no such registry");
                continue;
            };
            if !named.insert(registry) {
                report.whole_registry(
                    registry,
                    &directory,
                    "the tags of the registry are sent twice",
                );
                continue;
            }
            if update.tags.len() > TAG_CAPACITY {
                report.whole_registry(
                    registry,
                    &directory,
                    "the registry has more tags than an id can number",
                );
                continue;
            }
            let mut seen = HashSet::with_capacity(update.tags.len());
            let mut rows = Vec::with_capacity(update.tags.len());
            for (tag, members) in &update.tags {
                let label = format!("#{tag}");
                if !seen.insert(tag) {
                    report.entry(registry, &label, TAG_DATA, "the tag is sent twice");
                    continue;
                }
                let mut numbers = Vec::with_capacity(members.len());
                for &member in members {
                    match u16::try_from(member)
                        .ok()
                        .filter(|&member| usize::from(member) < names.len())
                    {
                        Some(member) => numbers.push(member),
                        None => report.entry(
                            registry,
                            &label,
                            TAG_DATA,
                            format_args!(
                                "member {member} is outside the registry of {} entries",
                                names.len()
                            ),
                        ),
                    }
                }
                rows.push((tag.clone(), numbers.into_boxed_slice()));
            }
            values.tags.insert(
                registry.clone(),
                Arc::new(assemble(names, rows.into_iter())),
            );
        }
        self.clone().with_values(values)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::Registry;
    use mcrs_minecraft_core::registry_key::RegistryKey;
    use mcrs_minecraft_core::rl;
    use mcrs_minecraft_core::tag_key::TagKey;
    use serde::{Deserialize, Serialize};

    #[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
    #[serde(deny_unknown_fields)]
    struct Probe {
        size: i32,
    }

    impl Probe {
        const KEY: RegistryKey<Probe> = RegistryKey::new(rl!("minecraft:test_probe"));
    }

    struct Unsynced;

    impl Unsynced {
        const KEY: RegistryKey<Unsynced> = RegistryKey::new(rl!("minecraft:test_unsynced"));
    }

    struct Fixed;

    impl Fixed {
        const KEY: RegistryKey<Fixed> = RegistryKey::new(rl!("minecraft:test_fixed"));
    }

    struct Other;

    impl Other {
        const KEY: RegistryKey<Other> = RegistryKey::new(rl!("minecraft:test_other"));
    }

    #[derive(Serialize)]
    struct Wrong {
        size: &'static str,
    }

    fn name(text: &str) -> Name {
        Name::read(text).unwrap()
    }

    fn probe(size: i32) -> NbtTag {
        mcrs_minecraft_nbt::to_nbt_tag(&Probe { size }).unwrap()
    }

    fn entry(text: &str, data: Option<NbtTag>) -> NetworkEntry {
        NetworkEntry {
            name: name(text),
            data,
        }
    }

    fn sent(registry: &str, entries: Vec<NetworkEntry>) -> NetworkRegistry {
        NetworkRegistry {
            registry: name(registry),
            entries,
        }
    }

    fn statics() -> RegistrySet {
        RegistrySet::new()
            .with_types([Probe::KEY.binding(), Unsynced::KEY.binding()])
            .unwrap()
    }

    fn declarations(non_empty: bool) -> WorldRegistries {
        let mut registries =
            WorldRegistries::new([Probe::KEY.location(), Unsynced::KEY.location()].map(Name::from));
        registries
            .parse::<Probe>(Probe::KEY.location())
            .sync_value::<Probe, _>(Probe::KEY.location(), Clone::clone)
            .receive::<Probe, _>(Probe::KEY.location(), |probe| (probe,))
            .parse::<Probe>(Unsynced::KEY.location());
        if non_empty {
            registries.non_empty(Probe::KEY.location());
        }
        registries
    }

    fn refused(registries: Vec<NetworkRegistry>) -> String {
        declarations(false)
            .from_network(&statics(), registries, &[], None)
            .err()
            .expect("the build is refused")
            .to_string()
    }

    #[test]
    fn a_probe_registry_is_built_in_the_order_it_was_sent() {
        let set = declarations(false)
            .from_network(
                &statics(),
                vec![sent(
                    "minecraft:test_probe",
                    vec![
                        entry("minecraft:zeta", Some(probe(1))),
                        entry("minecraft:alpha", Some(probe(2))),
                    ],
                )],
                &[],
                None,
            )
            .unwrap_or_else(|report| panic!("{report}"));
        let table = set.table("minecraft:test_probe").unwrap();
        assert_eq!(table.number("minecraft:zeta"), Some(0));
        assert_eq!(table.number("minecraft:alpha"), Some(1));
        assert_eq!(
            set.column::<Probe>("minecraft:test_probe").unwrap(),
            [Probe { size: 1 }, Probe { size: 2 }]
        );
    }

    #[test]
    fn a_received_registry_the_client_does_not_sync_fails() {
        for registry in ["minecraft:test_unsynced", "minecraft:test_nowhere"] {
            let text = refused(vec![sent(registry, vec![entry("minecraft:one", None)])]);
            assert!(text.contains(registry), "{text}");
            assert!(text.contains("does not sync"), "{text}");
        }
    }

    #[test]
    fn a_repeated_entry_in_one_packet_fails() {
        let text = refused(vec![sent(
            "minecraft:test_probe",
            vec![
                entry("minecraft:one", Some(probe(1))),
                entry("minecraft:one", Some(probe(2))),
            ],
        )]);
        assert!(
            text.contains("minecraft:test_probe/minecraft:one"),
            "{text}"
        );
        assert!(text.contains("sent twice"), "{text}");
    }

    #[test]
    fn an_entry_whose_nbt_does_not_decode_fails() {
        let wrong = mcrs_minecraft_nbt::to_nbt_tag(&Wrong { size: "text" }).unwrap();
        let text = refused(vec![sent(
            "minecraft:test_probe",
            vec![
                entry("minecraft:good", Some(probe(1))),
                entry("minecraft:bad", Some(wrong)),
            ],
        )]);
        assert!(
            text.contains("minecraft:test_probe/minecraft:bad"),
            "{text}"
        );
        assert!(text.contains("invalid type"), "{text}");
        assert!(!text.contains("minecraft:good"), "{text}");
    }

    #[test]
    fn a_registry_declared_non_empty_that_the_server_omits_fails() {
        let report = declarations(true)
            .from_network(&statics(), Vec::new(), &[], None)
            .err()
            .expect("the build is refused")
            .to_string();
        assert!(
            report.contains("Registry must be non-empty: minecraft:test_probe"),
            "{report}"
        );
    }

    fn registry<R>(key: RegistryKey<R>, names: &[&str]) -> Registry<R> {
        Registry::new(key, names.iter().map(|text| name(text))).unwrap()
    }

    fn tag_statics() -> RegistrySet {
        RegistrySet::new()
            .with(registry(
                Fixed::KEY,
                &["minecraft:a", "minecraft:b", "minecraft:c"],
            ))
            .unwrap()
            .with(registry(Other::KEY, &["minecraft:x", "minecraft:y"]))
            .unwrap()
    }

    fn tags_of(registry: &str, tags: &[(&str, &[i32])]) -> NetworkTags {
        NetworkTags {
            registry: name(registry),
            tags: tags
                .iter()
                .map(|(tag, members)| (name(tag), members.to_vec()))
                .collect(),
        }
    }

    fn members(set: &RegistrySet, tag: &str) -> Vec<usize> {
        let tags = set.tags::<Fixed>().unwrap();
        let tag = tags
            .get(&TagKey::<Fixed, _>::from_location(name(tag)))
            .expect("the tag exists");
        tags.members(tag).map(|id| id.index()).collect()
    }

    #[test]
    fn received_tags_keep_the_received_member_order() {
        let set = tag_statics()
            .with_network_tags(&[tags_of(
                "minecraft:test_fixed",
                &[("minecraft:t", &[2, 0, 1])],
            )])
            .unwrap_or_else(|report| panic!("{report}"));
        assert_eq!(members(&set, "minecraft:t"), [2, 0, 1]);
        let tags = set.tags::<Fixed>().unwrap();
        let fixed = set.registry::<Fixed>().unwrap();
        let tag = tags
            .get(&TagKey::<Fixed, _>::from_location(name("minecraft:t")))
            .unwrap();
        for id in fixed.ids() {
            assert!(tags.contains(tag, id));
        }
    }

    #[test]
    fn a_tag_member_outside_its_registry_fails() {
        for member in [3, -1, 65536, i32::MAX] {
            let report = tag_statics()
                .with_network_tags(&[tags_of(
                    "minecraft:test_fixed",
                    &[("minecraft:t", &[0, member])],
                )])
                .err()
                .expect("the member is refused")
                .to_string();
            assert!(
                report.contains("minecraft:test_fixed/#minecraft:t"),
                "{report}"
            );
            assert!(report.contains(&format!("member {member} ")), "{report}");
        }
    }

    #[test]
    fn an_update_replaces_only_the_registries_it_names() {
        let first = tag_statics()
            .with_network_tags(&[
                tags_of("minecraft:test_fixed", &[("minecraft:t", &[0])]),
                tags_of("minecraft:test_other", &[("minecraft:u", &[1])]),
            ])
            .unwrap();
        let tables = |set: &RegistrySet| {
            (
                Arc::clone(set.tag_table("minecraft:test_fixed").unwrap()),
                Arc::clone(set.tag_table("minecraft:test_other").unwrap()),
            )
        };
        let (fixed, other) = tables(&first);

        let second = first
            .with_network_tags(&[tags_of("minecraft:test_fixed", &[("minecraft:t", &[1, 2])])])
            .unwrap();
        let (new_fixed, kept_other) = tables(&second);
        assert!(!Arc::ptr_eq(&fixed, &new_fixed));
        assert!(Arc::ptr_eq(&other, &kept_other));
        assert_eq!(members(&second, "minecraft:t"), [1, 2]);

        let third = second.with_network_tags(&[]).unwrap();
        let (same_fixed, same_other) = tables(&third);
        assert!(Arc::ptr_eq(&new_fixed, &same_fixed));
        assert!(Arc::ptr_eq(&kept_other, &same_other));
    }

    #[test]
    fn a_tag_for_a_registry_the_set_lacks_fails() {
        let report = tag_statics()
            .with_network_tags(&[tags_of("minecraft:test_nowhere", &[("minecraft:t", &[0])])])
            .err()
            .expect("the registry is refused")
            .to_string();
        assert!(report.contains("minecraft:test_nowhere"), "{report}");
    }
}
