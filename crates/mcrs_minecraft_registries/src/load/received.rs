use std::borrow::Cow;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::Arc;

use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_datapack::{PackIndex, PackStack};
use mcrs_minecraft_nbt::tag::NbtTag;

use super::ctx::Slot;
use super::declaration::{Declaration, EntrySource, Kind, Source};
use super::loader::{
    assemble, check_declarations, decode, empty_registry, empty_slots, fail, slots_by_key, too_many,
};
use crate::registries::{Registries, Stage};
use crate::diagnostics::{Diagnostic, Diagnostics};
use crate::registry::Names;
use crate::tags::TagTable;

/// What a server sent of its registries during one configuration.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Received {
    /// Each registry's entries in the order sent, a registry sent in several packets already
    /// joined. An entry's data is absent where the server left it to a pack the client knows.
    pub registries: Vec<(ResourceLocation, Vec<(ResourceLocation, Option<NbtTag>)>)>,
    /// The tags of each registry, as numbers of the entries the registry holds.
    pub tags: Vec<(ResourceLocation, Vec<(ResourceLocation, Vec<i32>)>)>,
}

impl Received {
    /// Replaces the tags of each registry `update` names.
    pub fn replace_tags(
        &mut self,
        update: Vec<(ResourceLocation, Vec<(ResourceLocation, Vec<i32>)>)>,
    ) {
        for (registry, tags) in update {
            match self.tags.iter_mut().find(|(held, _)| *held == registry) {
                Some((_, held)) => *held = tags,
                None => self.tags.push((registry, tags)),
            }
        }
    }
}

/// Builds the registries a client holds from what a server sent. A synced registry holds the
/// entries sent, numbered in their order, and an entry sent without data takes the data of the
/// same entry of `known`. A registry read by its names alone holds the names sent. Every tag comes
/// from the received tags, and every other data registry is empty. Declaration validators do not
/// run: they check what a data pack load needs, such as the features a biome places, which a
/// server never sends.
pub fn load_received(
    declarations: &[Declaration],
    received: &Received,
    known: Option<&Registries>,
) -> Result<(Registries, Diagnostics), Diagnostics> {
    let diagnostics = Diagnostics::default();
    let errors = check_declarations(declarations);
    if !errors.is_empty() {
        return Err(fail(diagnostics, errors));
    }
    let mut slots = empty_slots(declarations);
    let by_key = slots_by_key(&slots);

    let (sources, errors) = name_slots(&mut slots, received, known);
    if !errors.is_empty() {
        return Err(fail(diagnostics, errors));
    }

    let stack = Arc::new(PackStack::default());
    let mut errors = decode(
        Stage::Reloadable,
        &mut slots,
        &by_key,
        &stack,
        (sources, BTreeMap::new()),
    );
    let synced = |slot: &&Slot<'_>| {
        slot.declaration
            .data_part()
            .is_some_and(|data| data.is_synced())
    };
    errors.extend(slots.iter().filter(synced).filter_map(empty_registry));
    if !errors.is_empty() {
        return Err(fail(diagnostics, errors));
    }
    match assemble(slots, stack, None) {
        Ok(registries) => Ok((registries, diagnostics)),
        Err(errors) => Err(fail(diagnostics, errors)),
    }
}

type Sources<'f> = Vec<(usize, Vec<EntrySource<'f>>)>;

/// The origin of an entry a server sent: no pack of the stack.
const RECEIVED: PackIndex = PackIndex::new(u16::MAX);

type Sent = [(ResourceLocation, Option<NbtTag>)];

/// Names every slot: a static registry from its declaration, a synced one and one read by its
/// names alone from the entries sent, and the tags of each from the received tags.
fn name_slots<'f>(
    slots: &mut [Slot<'_>],
    received: &'f Received,
    known: Option<&Registries>,
) -> (Sources<'f>, Vec<Diagnostic>) {
    let sent: HashMap<&ResourceLocation, &Sent> = received
        .registries
        .iter()
        .map(|(key, entries)| (key, entries.as_slice()))
        .collect();
    let mut errors = Vec::new();
    let mut sources = Vec::new();
    for (at, slot) in slots.iter_mut().enumerate() {
        let data = slot.declaration.data_part();
        let names = match &slot.declaration.kind {
            Kind::Fixed(names) => Names::new(names.clone(), Vec::new()),
            Kind::Names | Kind::Data(_) => {
                let entries = sent
                    .get(slot.key())
                    .filter(|_| data.is_none_or(|data| data.is_synced()))
                    .copied()
                    .unwrap_or_default();
                let mut seen = HashSet::new();
                let mut names = Vec::with_capacity(entries.len());
                let mut entry_sources = Vec::with_capacity(entries.len());
                for (name, tag) in entries {
                    if !seen.insert(name) {
                        errors.push(entry_error(slot.key(), name, "the entry is sent twice"));
                        continue;
                    }
                    names.push(name.clone());
                    let Some(data) = data else {
                        continue;
                    };
                    let tag = match tag {
                        Some(tag) => Cow::Borrowed(tag),
                        None => match known.and_then(|known| data.project(known, name)) {
                            Some(Ok(tag)) => Cow::Owned(tag),
                            Some(Err(message)) => {
                                errors.push(entry_error(slot.key(), name, &message));
                                continue;
                            }
                            None => {
                                errors.push(entry_error(
                                    slot.key(),
                                    name,
                                    "the entry is sent without data and no local pack holds it",
                                ));
                                continue;
                            }
                        },
                    };
                    entry_sources.push(EntrySource {
                        pack: RECEIVED,
                        source: Source::Network(tag),
                    });
                }
                if data.is_some() {
                    sources.push((at, entry_sources));
                }
                let origins = vec![RECEIVED; names.len()];
                Names::new(names, origins)
            }
        };
        let names = match names {
            Ok(names) => Arc::new(names),
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

fn entry_error(registry: &ResourceLocation, entry: &ResourceLocation, message: &str) -> Diagnostic {
    Diagnostic::Entry {
        registry: registry.clone(),
        entry: entry.clone(),
        pack: None,
        file: None,
        field: None,
        message: message.to_owned(),
    }
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
