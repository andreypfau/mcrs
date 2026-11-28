use mcrs_minecraft_brain::PanicCauses;
use mcrs_minecraft_entity::damage_type::DamageType;
use mcrs_minecraft_registries::CurrentRegistries;
use mcrs_minecraft_resource::TagKey;

pub(crate) fn panic_causes_in(
    registries: &CurrentRegistries,
    panic_causes: TagKey<'static, DamageType>,
) -> PanicCauses {
    registries
        .tags::<DamageType>()
        .and_then(|tags| {
            let tag = tags.id(panic_causes)?;
            Some(PanicCauses::new(
                tags.members(tag)
                    .filter_map(|id| u32::try_from(id.index()).ok()),
            ))
        })
        .unwrap_or_default()
}
