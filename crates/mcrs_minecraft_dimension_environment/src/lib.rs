#![cfg_attr(
    test,
    expect(
        clippy::disallowed_macros,
        clippy::disallowed_types,
        reason = "test input is written as JSON"
    )
)]

pub mod dimension_type;
pub mod environment;

mcrs_minecraft_registries::read_with_serde!(crate::dimension_type::DimensionTypeFile);

mcrs_minecraft_registries::declare_registries!(
    crate::dimension_type::DimensionTypeFile => mcrs_minecraft_dimension::keys::DIMENSION_TYPE, World;
);
