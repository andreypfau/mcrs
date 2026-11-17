#![expect(
    clippy::panic,
    clippy::unwrap_used,
    reason = "a developer tool and a test report a failure by panicking"
)]

pub mod artefact;
pub mod shapes;
