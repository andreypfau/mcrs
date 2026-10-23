use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_datapack::PackStack;
use mcrs_minecraft_nbt::tag::NbtTag;

use crate::declaration::{Declaration, Source};
use crate::diagnostics::{Diagnostic, Diagnostics};
use crate::load::{Filled, check, stages};
use crate::registries::{Kind, Registries};

/// Each registry's tags as sent: the tag names with the numbers of their members.
pub type ReceivedTags = Vec<(ResourceLocation, Vec<(ResourceLocation, Vec<i32>)>)>;

/// What a server sent of its registries during one configuration.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Received {
    /// Each registry's entries in the order sent, a registry sent in several packets already
    /// joined. An entry's data is absent where the server left it to a pack the client knows.
    pub registries: Vec<(ResourceLocation, Vec<(ResourceLocation, Option<NbtTag>)>)>,
    /// The tags of each registry, as numbers of the entries the registry holds.
    pub tags: ReceivedTags,
}

impl Received {
    /// Replaces the tags of each registry `update` names.
    pub fn replace_tags(&mut self, update: ReceivedTags) {
        for (registry, tags) in update {
            match self.tags.iter_mut().find(|(held, _)| *held == registry) {
                Some((_, held)) => *held = tags,
                None => self.tags.push((registry, tags)),
            }
        }
    }
}

impl Registries {
    /// Builds the registries a client holds from what a server sent. A synced registry holds the
    /// entries sent, numbered in their order, and an entry sent without data takes the data of the
    /// same entry of `known`. A registry read by its names alone holds the names sent. Every tag
    /// comes from the received tags, and every other data registry is empty. Declaration
    /// validators do not run: they check what a data pack load needs, such as the features a
    /// biome places, which a server never sends.
    pub fn receive(
        declarations: Vec<Declaration>,
        received: &Received,
        known: Option<&Registries>,
    ) -> Result<(Registries, Diagnostics), Diagnostics> {
        check(&declarations)?;
        let sent: HashMap<&ResourceLocation, &[(ResourceLocation, Option<NbtTag>)]> =
            (received.registries.iter())
                .map(|(key, entries)| (key, entries.as_slice()))
                .collect();
        let stack = Arc::new(PackStack::default());
        let loading = Registries::new(Vec::new(), Arc::clone(&stack), stack);
        stages(
            declarations,
            loading,
            true,
            Diagnostics::default(),
            |declaration, report| {
                let key = &declaration.key;
                let (listed, decoded) = match &declaration.kind {
                    Kind::Fixed => (None, false),
                    Kind::Names => (sent.get(key), false),
                    Kind::Data(codec) => (sent.get(key).filter(|_| codec.synced()), true),
                };
                let entries = listed.copied().unwrap_or_default();
                let mut names = std::mem::take(&mut declaration.names);
                let mut sources = Vec::with_capacity(entries.len());
                let mut seen = HashSet::new();
                for (name, data) in entries {
                    if !seen.insert(name) {
                        report
                            .errors
                            .push(entry_error(key, name, "the entry is sent twice"));
                        continue;
                    }
                    names.push(name.clone());
                    if !decoded {
                        continue;
                    }
                    let data = match data {
                        Some(data) => Ok(data.clone()),
                        None => known_entry(known, key, name),
                    };
                    match data {
                        Ok(data) => sources.push(Source::Sent(data)),
                        Err(message) => report.errors.push(entry_error(key, name, &message)),
                    }
                }
                let names = report.names(key, names, Vec::new())?;
                let tags = received_tags(key, &received.tags, names.len());
                Some(Filled {
                    tags: report.tags(key, names.len(), tags)?,
                    names,
                    kept: None,
                    sources: decoded.then_some(sources),
                })
            },
        )
    }
}

fn known_entry(
    known: Option<&Registries>,
    registry: &ResourceLocation,
    name: &ResourceLocation,
) -> Result<NbtTag, String> {
    let absent = || "the entry is sent without data and no local pack holds it".to_owned();
    let table = known.and_then(|known| known.table(registry.as_str()));
    let (Some(known), Some(table)) = (known, table) else {
        return Err(absent());
    };
    let (Kind::Data(codec), Some(values)) = (&table.kind, &table.registry) else {
        return Err(absent());
    };
    let at = table.names.get(name).ok_or_else(absent)?;
    codec.encode(values, usize::from(at), known)
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
) -> Vec<(ResourceLocation, Option<Vec<u16>>)> {
    received
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
        .collect()
}
