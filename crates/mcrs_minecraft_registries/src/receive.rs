use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use mcrs_minecraft_datapack::ReadError;
use mcrs_minecraft_nbt::tag::NbtTag;
use mcrs_minecraft_resource::ResourceLocation;

use crate::declaration::{Declaration, Source, entry_path};
use crate::diagnostics::{Diagnostic, Diagnostics};
use crate::load::{Filled, stages};
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
    /// entries sent, numbered in their order. An entry sent without data is decoded in the network
    /// form from the file `known` reads at the path of the entry's file in the packs the server
    /// negotiated. A registry read by its names alone holds the names sent. Every tag comes from
    /// the received tags, and every other data registry is empty. Declaration validators do not
    /// run: they check what a data pack load needs, such as the features a biome places, which a
    /// server never sends.
    pub fn receive(
        declarations: Vec<Declaration>,
        received: &Received,
        known: Option<&dyn Fn(&str) -> Result<Vec<u8>, ReadError>>,
    ) -> Result<(Registries, Diagnostics), Diagnostics> {
        let sent: HashMap<&ResourceLocation, &[(ResourceLocation, Option<NbtTag>)]> =
            (received.registries.iter())
                .map(|(key, entries)| (key, entries.as_slice()))
                .collect();
        stages(
            declarations,
            Registries::new(Vec::new(), Arc::default(), Arc::default()),
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
                        let error = Diagnostic::entry(key, name, "the entry is sent twice");
                        report.errors.push(error);
                        continue;
                    }
                    names.push(name.clone());
                    if !decoded {
                        continue;
                    }
                    let source = match (data, known) {
                        (Some(data), _) => Ok(Source::Sent(data)),
                        (None, Some(read)) => read(&entry_path(key.path(), name))
                            .map(Source::Known)
                            .map_err(|error| error.to_string()),
                        (None, None) => Err("the entry is sent without data and no known pack \
                                             is negotiated"
                            .to_owned()),
                    };
                    match source {
                        Ok(source) => sources.push(source),
                        Err(message) => report.errors.push(Diagnostic::entry(key, name, message)),
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
