use glam::DVec3;
use mcrs_minecraft_environment::AttributeValue;
use mcrs_minecraft_environment::environment_attribute_keys::EnvironmentAttribute;
use mcrs_minecraft_predicate::NoWorld;

/// The environment attributes of the world of an evaluation, as the provider
/// `environment_attribute` and the condition `environment_attribute_check` read them
/// (LOOT-NUM-016, LOOT-CND-019).
pub trait EnvironmentView {
    /// The value of `attribute`, in the type of the attribute. `position` is the `origin` for a
    /// positional attribute and `None` for the dimension value of an attribute that is not
    /// positional. `None` means that the host does not have the value; the reader then takes the
    /// default value of the attribute.
    // chisle: a value that holds text or a list allocates in the host on each read; a borrowed or host-compared form lifts it.
    fn environment_value(
        &self,
        attribute: EnvironmentAttribute,
        position: Option<DVec3>,
    ) -> Option<AttributeValue>;
}

impl EnvironmentView for NoWorld {
    fn environment_value(
        &self,
        _: EnvironmentAttribute,
        _: Option<DVec3>,
    ) -> Option<AttributeValue> {
        None
    }
}
