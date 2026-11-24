use bevy_ecs::resource::Resource;
use bevy_ecs::system::Res;
use bevy_ecs::system::ResMut;
use mcrs_minecraft_entity::damage_type_keys;
use mcrs_minecraft_entity::registry_keys::DAMAGE_TYPE;
use mcrs_minecraft_registries::CurrentRegistries;

#[derive(Resource, Clone, Copy, Debug, Default)]
pub(crate) struct CrowdDamageType(pub(crate) Option<u32>);

pub(crate) fn find_crowd_damage_type(
    mut found: ResMut<CrowdDamageType>,
    registries: Res<CurrentRegistries>,
) {
    let id = registries
        .id_of(
            DAMAGE_TYPE.location(),
            damage_type_keys::CRAMMING.location(),
        )
        .map(u32::from);
    if found.0 != id {
        found.0 = id;
    }
    if id.is_none() {
        tracing::error!("the registries hold no damage type `cramming`; crowds take no damage");
    }
}
