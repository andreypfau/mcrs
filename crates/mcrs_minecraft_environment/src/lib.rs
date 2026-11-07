#![cfg_attr(
    test,
    expect(
        clippy::disallowed_macros,
        clippy::disallowed_types,
        reason = "test input is written as JSON"
    )
)]

pub mod attribute;
#[rustfmt::skip]
pub mod activity_keys;
#[rustfmt::skip]
pub mod environment_attribute_keys;
#[rustfmt::skip]
pub mod registry_keys;
#[rustfmt::skip]
pub mod test_environment_definition_type_keys;
#[rustfmt::skip]
pub mod test_instance_type_keys;
#[rustfmt::skip]
pub mod timeline_keys;
#[rustfmt::skip]
pub mod timeline_tag_keys;
#[rustfmt::skip]
pub mod world_clock_keys;
pub mod timeline;
pub mod world_clock;

#[cfg(test)]
mod load_rules;
#[cfg(test)]
mod parity;
