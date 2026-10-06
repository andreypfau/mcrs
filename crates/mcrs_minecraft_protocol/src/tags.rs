use crate::RegistryId;
use crate::packets::configuration::clientbound::{RegistryTags, TagGroup};
use mcrs_minecraft_registry::Registered;
use mcrs_minecraft_registry::{Registry, Tags};

pub fn tags_payload<R: Registered>(tags: &Tags<R>) -> RegistryTags<'static> {
    RegistryTags {
        registry: R::REGISTRY.location().into(),
        tags: tags
            .tag_ids()
            .map(|tag| TagGroup {
                name: tags.name(tag).clone().into(),
                entries: tags.members(tag).map(RegistryId::from).collect(),
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
                .filter_map(|&RegistryId(number)| registry.id(number))
                .collect();
            (group.name.clone().into(), members)
        })
        .collect();
    Tags::from_members(registry, tags)
}
