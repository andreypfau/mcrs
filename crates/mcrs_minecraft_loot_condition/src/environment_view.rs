use mcrs_minecraft_predicate::NoWorld;

use glam::DVec3;
use mcrs_minecraft_environment::AttributeValue;
use mcrs_minecraft_environment::environment_attribute_keys::EnvironmentAttribute;

pub trait EnvironmentView {
    fn environment_value(
        &self,
        _attribute: EnvironmentAttribute,
        _position: Option<DVec3>,
    ) -> Option<AttributeValue> {
        None
    }
}

impl EnvironmentView for NoWorld {}
