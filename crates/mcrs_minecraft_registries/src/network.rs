use mcrs_minecraft_nbt::serializer::WriteAdaptor;
use mcrs_minecraft_resource::ResourceLocationBuf;

use crate::Registries;
use crate::declaration::Codec;
use crate::diagnostics::{Diagnostic, Diagnostics};
use crate::registries::{Kind, Table};
use crate::tags::TagTable;

#[derive(Debug)]
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
    pub fn new(registries: &Registries) -> Result<Self, Diagnostics> {
        let mut synced = Vec::new();
        let mut synced_tags: Vec<(&ResourceLocationBuf, &TagTable)> = Vec::new();
        let mut static_tags: Vec<(&ResourceLocationBuf, &TagTable)> = Vec::new();
        let mut errors = Vec::new();
        for table in registries.tables() {
            if table.key.namespace() != "minecraft" {
                continue;
            }
            let tags =
                (table.tags.resolved().next().is_some()).then_some((&table.key, &*table.tags));
            match &table.kind {
                Kind::Fixed => static_tags.extend(tags),
                Kind::Data(codec) if codec.synced() => {
                    synced.extend(registry_data(
                        registries,
                        table,
                        codec.as_ref(),
                        &mut errors,
                    ));
                    synced_tags.extend(tags);
                }
                Kind::Names | Kind::Data(_) => {}
            }
        }
        if !errors.is_empty() {
            return Err(Diagnostics::default().with_report(errors));
        }
        synced_tags.append(&mut static_tags);
        Ok(NetworkSnapshot {
            registries: synced.into_boxed_slice(),
            tags: update_tags(&synced_tags),
        })
    }

    pub fn registry_data(&self, known: bool) -> impl Iterator<Item = &[u8]> {
        self.registries.iter().map(move |registry| {
            if known {
                &*registry.by_reference
            } else {
                &*registry.full
            }
        })
    }

    pub fn tags(&self) -> &[u8] {
        &self.tags
    }
}

fn registry_data(
    registries: &Registries,
    table: &Table,
    codec: &dyn Codec,
    errors: &mut Vec<Diagnostic>,
) -> Option<SyncedRegistry> {
    let Some(values) = &table.registry else {
        errors.push(Diagnostic::Declaration {
            registry: table.key.clone(),
            message: "the synced registry was not loaded".to_owned(),
        });
        return None;
    };
    let stack = registries.stack(table.stage);
    let mut entries = Vec::with_capacity(table.names.len());
    for (name, number) in table.names.all().iter().zip(0..=u16::MAX) {
        let mut data = Vec::new();
        let encoded = (codec.encode(values, usize::from(number), registries)).and_then(|tag| {
            (tag.serialize(&mut WriteAdaptor::new(&mut data))).map_err(|e| e.to_string())
        });
        if let Err(message) = encoded {
            let message = format!("cannot be sent to a client: {message}");
            errors.push(Diagnostic::entry(&table.key, name, message));
            return None;
        }
        let origin = table.names.origin(number).and_then(|pack| stack.get(pack));
        entries.push((
            name,
            data,
            origin.is_some_and(|pack| pack.known_pack().is_some()),
        ));
    }
    let body = |known: bool| {
        let mut out = Vec::new();
        string(&mut out, table.key.as_str());
        var_int(&mut out, entries.len());
        for (name, data, from_known_pack) in &entries {
            string(&mut out, name.as_str());
            let present = !(known && *from_known_pack);
            out.push(u8::from(present));
            if present {
                out.extend_from_slice(data);
            }
        }
        out.into_boxed_slice()
    };
    Some(SyncedRegistry {
        full: body(false),
        by_reference: body(true),
    })
}

fn update_tags(tagged: &[(&ResourceLocationBuf, &TagTable)]) -> Box<[u8]> {
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
