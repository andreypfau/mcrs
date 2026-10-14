#[path = "../../src/tests/support.rs"]
#[expect(
    dead_code,
    reason = "the benchmark compiles the shared test helpers whole and calls only the few it needs"
)]
pub mod support;
