use bevy_ecs::query::QueryData;
use mcrs_minecraft_entity_attribute::AttributeInstances;
use mcrs_minecraft_entity_attribute::attribute_keys::Attribute;
use mcrs_minecraft_pose::{Dimensions, Pose};

#[derive(QueryData)]
pub struct PlayerBody {
    pub pose: &'static Pose,
    pub attributes: Option<&'static AttributeInstances>,
}

impl PlayerBodyItem<'_, '_> {
    #[must_use]
    pub fn dimensions(&self) -> Dimensions {
        player_dimensions(*self.pose, self.attributes)
    }
}

#[must_use]
pub fn player_dimensions(pose: Pose, attributes: Option<&AttributeInstances>) -> Dimensions {
    pose.player_dimensions().scaled(player_scale(attributes))
}

#[must_use]
pub fn player_scale(attributes: Option<&AttributeInstances>) -> f32 {
    attributes
        .and_then(|attributes| attributes.value(Attribute::Scale))
        .unwrap_or(1.0) as f32
}
