use std::borrow::Cow;

use anyhow::Context as _;
use mcrs_minecraft_network::join::Configuration;
use mcrs_minecraft_protocol::Decode;
use mcrs_minecraft_protocol::packets::configuration::clientbound::{
    ClientboundRegistryData, ClientboundUpdateTags, RegistryTags,
};
use mcrs_minecraft_registries::{Received, ReceivedTags};
use mcrs_minecraft_resource::ResourceLocation;

/// What a server sent of its registries during one configuration: a registry sent in several
/// packets is joined in the order sent, and a later tag packet replaces the tags of the
/// registries it names.
pub(crate) fn received(configuration: &Configuration) -> anyhow::Result<Received> {
    let mut received = Received::default();
    for body in &configuration.registry_data {
        let data =
            ClientboundRegistryData::decode(&mut &body[..]).context("reading the registry data")?;
        collect_registry(&mut received, data);
    }
    for body in &configuration.tags {
        let update =
            ClientboundUpdateTags::decode(&mut &body[..]).context("reading the update tags")?;
        received.replace_tags(received_tags(update.registries));
    }
    Ok(received)
}

fn collect_registry(received: &mut Received, data: ClientboundRegistryData<'_>) {
    let registry: ResourceLocation = data.registry.into();
    let entries = data
        .entries
        .into_iter()
        .map(|entry| (entry.id.into(), entry.data.map(Cow::into_owned)));
    match received
        .registries
        .iter_mut()
        .find(|(collected, _)| *collected == registry)
    {
        Some((_, collected)) => collected.extend(entries),
        None => received.registries.push((registry, entries.collect())),
    }
}

pub(crate) fn received_tags(update: Vec<RegistryTags<'_>>) -> ReceivedTags {
    update
        .into_iter()
        .map(|registry| {
            let tags = registry
                .tags
                .into_iter()
                .map(|group| {
                    let members = group.entries.into_iter().map(|member| member.0).collect();
                    (group.name.into(), members)
                })
                .collect();
            (registry.registry.into(), tags)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use mcrs_minecraft_nbt::tag::NbtTag;
    use mcrs_minecraft_protocol::packets::configuration::clientbound::TagGroup;
    use mcrs_minecraft_protocol::registry::Entry;
    use mcrs_minecraft_protocol::{Encode, VarInt};
    use serde::Serialize;
    use std::sync::Arc;

    #[derive(Serialize)]
    struct Probe {
        size: i32,
    }

    const PROBE_NAME: &str = "minecraft:test_probe";

    fn name(text: &str) -> ResourceLocation<Arc<str>> {
        ResourceLocation::read(text).unwrap()
    }

    fn probe(size: i32) -> NbtTag {
        mcrs_minecraft_nbt::to_nbt_tag(&Probe { size }).unwrap()
    }

    fn body<P: Encode>(packet: &P) -> bytes::Bytes {
        let mut body = Vec::new();
        packet.encode(&mut body).unwrap();
        body.into()
    }

    fn probe_data(entries: &[(&str, Option<i32>)]) -> bytes::Bytes {
        body(&ClientboundRegistryData {
            registry: ResourceLocation::read_cow(PROBE_NAME.to_owned()).unwrap(),
            entries: entries
                .iter()
                .map(|(entry, size)| Entry {
                    id: ResourceLocation::read_cow((*entry).to_owned()).unwrap(),
                    data: size.map(|size| Cow::Owned(probe(size))),
                })
                .collect(),
        })
    }

    fn probe_tags(tag: &str, members: &[i32]) -> bytes::Bytes {
        body(&ClientboundUpdateTags {
            registries: vec![RegistryTags {
                registry: ResourceLocation::read_cow(PROBE_NAME.to_owned()).unwrap(),
                tags: vec![TagGroup {
                    name: ResourceLocation::read_cow(tag.to_owned()).unwrap(),
                    entries: members.iter().copied().map(VarInt).collect(),
                }],
            }],
        })
    }

    #[test]
    fn a_registry_sent_in_parts_is_joined_and_the_last_tags_win() {
        let configuration = Configuration {
            registry_data: vec![
                probe_data(&[("minecraft:zeta", Some(0)), ("minecraft:alpha", None)]),
                probe_data(&[("minecraft:beta", Some(2))]),
            ],
            tags: vec![
                probe_tags("minecraft:old", &[0]),
                probe_tags("minecraft:t", &[2, 0]),
            ],
            vanilla_known: false,
        };

        assert_eq!(
            received(&configuration).unwrap(),
            Received {
                registries: vec![(
                    name(PROBE_NAME),
                    vec![
                        (name("minecraft:zeta"), Some(probe(0))),
                        (name("minecraft:alpha"), None),
                        (name("minecraft:beta"), Some(probe(2))),
                    ]
                )],
                tags: vec![(name(PROBE_NAME), vec![(name("minecraft:t"), vec![2, 0])])],
            }
        );
    }
}
