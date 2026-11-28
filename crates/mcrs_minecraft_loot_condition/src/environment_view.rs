use glam::DVec3;
use mcrs_minecraft_environment::AttributeValue;
use mcrs_minecraft_environment::environment_attribute_keys::EnvironmentAttribute;
use mcrs_minecraft_predicate::BlockHost;

pub trait EnvironmentView {
    fn environment_value(
        &self,
        attribute: EnvironmentAttribute,
        position: Option<DVec3>,
    ) -> Option<AttributeValue>;
}

impl<F, E> EnvironmentView for BlockHost<F, E> {
    fn environment_value(
        &self,
        _: EnvironmentAttribute,
        _: Option<DVec3>,
    ) -> Option<AttributeValue> {
        None
    }
}
