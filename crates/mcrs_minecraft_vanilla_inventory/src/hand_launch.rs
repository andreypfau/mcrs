use mcrs_minecraft_engine_world::entity::physics::Transform;
use mcrs_minecraft_entity_key::BirthSource;
use mcrs_minecraft_entity_key::EntityUuid;
use mcrs_minecraft_item_entity::Cause;
use mcrs_minecraft_item_entity::HandVelocity;
use mcrs_minecraft_item_entity::ItemEntity;
use mcrs_minecraft_vanilla_item_entity::Form;
use mcrs_minecraft_vanilla_item_entity::Launch;

pub(crate) fn hand_launch(
    transform: Transform,
    eye_height: f32,
    source: u128,
    cause: Cause,
) -> Launch {
    Launch {
        source: BirthSource::Entity(EntityUuid::from_u128(source)),
        form: Form::Hand {
            feet: transform.translation,
            eye_height,
            velocity: HandVelocity::Look {
                yaw: transform.rotation.yaw(),
                pitch: transform.rotation.pitch(),
            },
        },
        values: ItemEntity::with_pickup_delay(cause.pickup_delay()),
    }
}
