use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::Arc;

use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_datapack::{PackIndex, PackStack};
use mcrs_minecraft_nbt::tag::NbtTag;

use super::ctx::Slot;
use super::declaration::{Declaration, EntrySource, Kind, Source};
use super::loader::{
    assemble, check_declarations, decode, empty_slots, fail, report_unresolved, slots_by_key,
    too_many,
};
use crate::registries::{Registries, Stage};
use crate::diagnostics::{Diagnostic, Diagnostics};
use crate::registry::Names;
use crate::tags::TagTable;

/// What a server sent of its registries during one configuration, with the data of every entry:
/// an entry the server sent without data is filled in by the client before it is loaded.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Received {
    /// Each synced registry's entries in the order sent, a registry sent in several packets
    /// already joined.
    pub registries: Vec<(ResourceLocation, Vec<(ResourceLocation, NbtTag)>)>,
    /// The tags of each registry, as numbers of the entries the registry holds.
    pub tags: Vec<(ResourceLocation, Vec<(ResourceLocation, Vec<i32>)>)>,
}

/// Builds the registries a client holds from what a server sent. A synced registry holds the
/// entries sent, numbered in their order; every tag comes from the received tags, and every other
/// data registry is empty. Declaration validators do not run: they check what a data pack load
/// needs, such as the features a biome places, which a server never sends.
pub fn load_received(
    declarations: &[Declaration],
    received: &Received,
) -> Result<(Registries, Diagnostics), Diagnostics> {
    let diagnostics = Diagnostics::default();
    let errors = check_declarations(declarations);
    if !errors.is_empty() {
        return Err(fail(diagnostics, errors));
    }
    let mut slots = empty_slots(declarations);
    let by_key = slots_by_key(&slots);

    let (sources, mut errors) = name_slots(&mut slots, received);
    if !errors.is_empty() {
        return Err(fail(diagnostics, errors));
    }

    let stack = Arc::new(PackStack::default());
    let missing = decode(
        Stage::Reloadable,
        &mut slots,
        &by_key,
        sources,
        &stack,
        &mut errors,
    );
    let mut unresolved = BTreeMap::new();
    for (at, missing) in missing {
        unresolved.entry(at).or_insert_with(Vec::new).push(missing);
    }
    report_unresolved(unresolved, &slots, &mut errors);
    for slot in &slots {
        if let (Some(data), Some(names)) = (slot.declaration.data_part(), &slot.names)
            && data.is_synced()
            && data.must_not_be_empty()
            && names.len() == 0
        {
            errors.push(Diagnostic::EmptyRegistry {
                registry: slot.key().clone(),
            });
        }
    }
    if !errors.is_empty() {
        return Err(fail(diagnostics, errors));
    }
    match assemble(slots, stack, None) {
        Ok(registries) => Ok((registries, diagnostics)),
        Err(errors) => Err(fail(diagnostics, errors)),
    }
}

type Sources<'f> = Vec<(usize, Vec<EntrySource<'f>>)>;

/// The origin of an entry a server sent with its data: no pack of the stack.
const RECEIVED: PackIndex = PackIndex::new(u16::MAX);

/// Names every slot: a static registry from its declaration, a synced one from the entries sent,
/// and the tags of each from the received tags.
fn name_slots<'f>(
    slots: &mut [Slot<'_>],
    received: &'f Received,
) -> (Sources<'f>, Vec<Diagnostic>) {
    let sent: HashMap<&ResourceLocation, &[(ResourceLocation, NbtTag)]> = received
        .registries
        .iter()
        .map(|(key, entries)| (key, entries.as_slice()))
        .collect();
    let mut errors = Vec::new();
    let mut sources = Vec::new();
    for (at, slot) in slots.iter_mut().enumerate() {
        let names = match (&slot.declaration.kind, slot.declaration.data_part()) {
            (Kind::Fixed(names), _) => Names::new(names.clone(), Vec::new()).map(Arc::new),
            (Kind::Names, _) => Names::new(Vec::new(), Vec::new()).map(Arc::new),
            (Kind::Data(_), Some(data)) => {
                let entries = sent
                    .get(slot.key())
                    .filter(|_| data.is_synced())
                    .copied()
                    .unwrap_or_default();
                let mut seen = HashSet::new();
                let mut names = Vec::with_capacity(entries.len());
                let mut entry_sources = Vec::with_capacity(entries.len());
                for (name, tag) in entries {
                    if !seen.insert(name) {
                        errors.push(Diagnostic::Entry {
                            registry: slot.key().clone(),
                            entry: name.clone(),
                            pack: None,
                            file: None,
                            field: None,
                            message: "the entry is sent twice".to_owned(),
                        });
                        continue;
                    }
                    names.push(name.clone());
                    entry_sources.push(EntrySource {
                        pack: RECEIVED,
                        source: Source::Network(tag),
                    });
                }
                let origins = vec![RECEIVED; entry_sources.len()];
                sources.push((at, entry_sources));
                Names::new(names, origins).map(Arc::new)
            }
            (Kind::Data(_), None) => Names::new(Vec::new(), Vec::new()).map(Arc::new),
        };
        let names = match names {
            Ok(names) => names,
            Err(count) => {
                errors.push(too_many(slot.key(), count));
                continue;
            }
        };
        match received_tags(slot.key(), &received.tags, names.len()) {
            Ok(tags) => slot.tags = Some(Arc::new(tags)),
            Err(count) => errors.push(Diagnostic::TooManyTags {
                registry: slot.key().clone(),
                count,
            }),
        }
        slot.names = Some(names);
    }
    (sources, errors)
}

/// Members numbered outside the registry are dropped, as the game drops them.
fn received_tags(
    registry: &ResourceLocation,
    received: &[(ResourceLocation, Vec<(ResourceLocation, Vec<i32>)>)],
    entries: usize,
) -> Result<TagTable, usize> {
    let tags = received
        .iter()
        .rfind(|(key, _)| key == registry)
        .map(|(_, tags)| tags.as_slice())
        .unwrap_or_default()
        .iter()
        .map(|(name, members)| {
            let members = members
                .iter()
                .filter_map(|&member| u16::try_from(member).ok())
                .filter(|&member| usize::from(member) < entries)
                .collect();
            (name.clone(), Some(members))
        })
        .collect();
    TagTable::new(entries, tags)
}
