#![expect(
    clippy::indexing_slicing,
    reason = "the panics in this crate are not yet replaced by errors or by code that cannot panic"
)]
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
