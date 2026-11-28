const SECTIONS_Y: i32 = 5;
const COLUMNS: i32 = 3;

include!("incremental_terrain/cases.rs");

#[expect(
    clippy::excessive_nesting,
    reason = "includes the shared cases one block deeper than the default run does"
)]
mod exhaustive {
    const SECTIONS_Y: i32 = 6;
    const COLUMNS: i32 = 5;

    include!("incremental_terrain/cases.rs");
}
