use mcrs_minecraft_core::ResourceLocation;

use crate::Registries;
use crate::load::{Declaration, Kind};
use crate::diagnostics::Diagnostic;
use crate::tags::TagTable;

/// The bodies of the configuration packets that carry the registries, without their packet ids,
/// encoded once per load.
#[derive(Debug, Default)]
pub struct NetworkSnapshot {
    registries: Box<[SyncedRegistry]>,
    tags: Box<[u8]>,
}

#[derive(Debug)]
struct SyncedRegistry {
    full: Box<[u8]>,
    by_reference: Box<[u8]>,
}

impl NetworkSnapshot {
    /// One registry data body per synced registry, in the declared order. With `known`, an entry
    /// whose winning pack has a known-pack identity leaves its data out.
    pub fn registry_data(&self, known: bool) -> impl Iterator<Item = &[u8]> {
        self.registries.iter().map(move |registry| {
            if known {
                &*registry.by_reference
            } else {
                &*registry.full
            }
        })
    }

    /// The update tags body: the tags of every static and synced registry that has one.
    pub fn tags(&self) -> &[u8] {
        &self.tags
    }
}

/// A synced entry, projected to the value the client decodes and encoded as network NBT.
pub(crate) struct SyncedEntry<'a> {
    pub(crate) name: &'a ResourceLocation,
    pub(crate) data: Vec<u8>,
    pub(crate) known: bool,
}

pub(crate) fn snapshot(
    declarations: &[Declaration],
    registries: &Registries,
) -> Result<NetworkSnapshot, Vec<Diagnostic>> {
    let mut synced = Vec::new();
    let mut synced_tags: Vec<(&ResourceLocation, &TagTable)> = Vec::new();
    let mut static_tags: Vec<(&ResourceLocation, &TagTable)> = Vec::new();
    let mut errors = Vec::new();
    for declaration in declarations {
        let tags = registries
            .table(declaration.key().as_str())
            .filter(|table| table.tags.resolved().next().is_some())
            .map(|table| (declaration.key(), &*table.tags));
        match &declaration.kind {
            Kind::Fixed(_) => static_tags.extend(tags),
            Kind::Names => {}
            Kind::Data(data) => match data.synced_entries(registries) {
                None => {}
                Some(Ok(entries)) => {
                    synced.push(registry_data(declaration.key(), &entries));
                    synced_tags.extend(tags);
                }
                Some(Err(error)) => errors.push(error),
            },
        }
    }
    if !errors.is_empty() {
        return Err(errors);
    }
    synced_tags.append(&mut static_tags);
    Ok(NetworkSnapshot {
        registries: synced.into_boxed_slice(),
        tags: update_tags(&synced_tags),
    })
}

fn registry_data(registry: &ResourceLocation, entries: &[SyncedEntry<'_>]) -> SyncedRegistry {
    let body = |known: bool| {
        let mut out = Vec::new();
        string(&mut out, registry.as_str());
        var_int(&mut out, entries.len());
        for entry in entries {
            string(&mut out, entry.name.as_str());
            let present = !(known && entry.known);
            out.push(u8::from(present));
            if present {
                out.extend_from_slice(&entry.data);
            }
        }
        out.into_boxed_slice()
    };
    SyncedRegistry {
        full: body(false),
        by_reference: body(true),
    }
}

fn update_tags(tagged: &[(&ResourceLocation, &TagTable)]) -> Box<[u8]> {
    let mut out = Vec::new();
    var_int(&mut out, tagged.len());
    for (registry, table) in tagged {
        string(&mut out, registry.as_str());
        var_int(&mut out, table.resolved().count());
        for (_, name, members) in table.resolved() {
            string(&mut out, name.as_str());
            var_int(&mut out, members.len());
            for &member in members {
                var_int(&mut out, usize::from(member));
            }
        }
    }
    out.into_boxed_slice()
}

/// A count or an id never exceeds what a registry can number, which fits a `VarInt`.
fn var_int(out: &mut Vec<u8>, value: usize) {
    let mut value = value as u32;
    loop {
        let byte = (value & 0x7F) as u8;
        value >>= 7;
        if value == 0 {
            out.push(byte);
            return;
        }
        out.push(byte | 0x80);
    }
}

fn string(out: &mut Vec<u8>, value: &str) {
    var_int(out, value.len());
    out.extend_from_slice(value.as_bytes());
}
