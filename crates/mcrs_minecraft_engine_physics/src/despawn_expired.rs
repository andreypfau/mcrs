use bevy_ecs::system::Commands;
use bevy_ecs::system::Res;
use mcrs_minecraft_engine_entity_motion::Expired;

pub(crate) fn has_expired(expired: Res<Expired>) -> bool {
    !expired.entries().is_empty()
}

pub(crate) fn despawn_expired(mut commands: Commands, expired: Res<Expired>) {
    for expiry in expired.entries() {
        if let Ok(mut entity) = commands.get_entity(expiry.entity) {
            entity.try_despawn();
        }
    }
}
