use glam::DVec3;
use mcrs_minecraft_environment::environment_attribute_keys::EnvironmentAttribute;
use mcrs_minecraft_predicate::NoWorld;

/// The environment attributes of the level of an evaluation, as the provider
/// `environment_attribute` reads them (LOOT-NUM-016, LOOT-CND-019).
///
/// `position` is the `origin` for a positional attribute and `None` for the dimension value of
/// an attribute that is not positional. `None` from the host gives the default value of the
/// attribute.
pub trait EnvironmentView {
    /// The value of an attribute of the type `integer`.
    fn environment_int(
        &self,
        attribute: EnvironmentAttribute,
        position: Option<DVec3>,
    ) -> Option<i32>;

    /// The value of an attribute of the type `float` or `angle_degrees`.
    fn environment_float(
        &self,
        attribute: EnvironmentAttribute,
        position: Option<DVec3>,
    ) -> Option<f32>;
}

impl EnvironmentView for NoWorld {
    fn environment_int(&self, _: EnvironmentAttribute, _: Option<DVec3>) -> Option<i32> {
        None
    }

    fn environment_float(&self, _: EnvironmentAttribute, _: Option<DVec3>) -> Option<f32> {
        None
    }
}
