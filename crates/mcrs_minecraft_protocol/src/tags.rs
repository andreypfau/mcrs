use crate::VarInt;
use crate::packets::configuration::clientbound::{RegistryTags, TagGroup};
use mcrs_minecraft_registry::Registered;
use mcrs_minecraft_registry::{Registry, TagTable, Tags};

pub fn tags_payload<R: Registered>(tags: &Tags<R>) -> RegistryTags<'static> {
    tags_payload_of(tags.table())
}

pub fn tags_payload_of(table: &TagTable) -> RegistryTags<'static> {
    RegistryTags {
        registry: table.registry().clone().into(),
        tags: table
            .names()
            .iter()
            .enumerate()
            .map(|(tag, name)| TagGroup {
                name: name.clone().into(),
                entries: table
                    .members(tag)
                    .iter()
                    .map(|&number| VarInt(i32::from(number)))
                    .collect(),
            })
            .collect(),
    }
}

pub fn tags_from_payload<R: 'static>(
    registry: &Registry<R>,
    payload: &RegistryTags<'_>,
) -> Tags<R> {
    let tags = payload
        .tags
        .iter()
        .map(|group| {
            let members = group
                .entries
                .iter()
                .filter_map(|&VarInt(number)| registry.id(u16::try_from(number).ok()?))
                .collect();
            (group.name.clone().into(), members)
        })
        .collect();
    Tags::from_members(registry, tags)
}
