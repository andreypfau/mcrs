//! The oracle artefacts and the shape table of the static block entities. A developer tool and
//! a test read them, so they may panic on a malformed input.
#![expect(
    clippy::panic,
    clippy::unwrap_used,
    reason = "a developer tool and a test report a failure by panicking"
)]

pub mod artefact;
pub mod shapes;
