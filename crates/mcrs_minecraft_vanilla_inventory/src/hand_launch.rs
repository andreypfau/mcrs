use mcrs_minecraft_engine_world::entity::physics::Transform;
use mcrs_minecraft_entity::EntityTypeData;
use mcrs_minecraft_entity_key::BirthSource;
use mcrs_minecraft_entity_key::EntityKey;
use mcrs_minecraft_item_entity::Cause;
use mcrs_minecraft_item_entity::HandVelocity;
use mcrs_minecraft_item_entity::ItemEntity;
use mcrs_minecraft_vanilla_item_entity::Form;
use mcrs_minecraft_vanilla_item_entity::Launch;

/// The launch of a stack from the hand of a player at `transform` along its look, with the pickup
/// delay of `cause` (IEN-SPN-001, IEN-SPN-002).
pub(crate) fn hand_launch(transform: Transform, source: u128, cause: Cause) -> Launch {
    Launch {
        source: BirthSource::Entity(EntityKey(source)),
        form: Form::Hand {
            feet: transform.translation,
            eye_height: EntityTypeData::PLAYER.eye_height,
            velocity: HandVelocity::Look {
                yaw: transform.rotation.yaw(),
                pitch: transform.rotation.pitch(),
            },
        },
        values: ItemEntity::with_pickup_delay(cause.pickup_delay()),
    }
}
