//! Columns of open terrain arriving over many ticks, lit through the queue and
//! the epoch budget rather than in one pass. Every surface block of an open
//! world sees the sky, so any cell the engine leaves below fifteen there is a
//! seam the incremental path failed to repair.

const SECTIONS_Y: i32 = 5;
const COLUMNS: i32 = 3;

include!("incremental_terrain/cases.rs");

mod exhaustive {
    const SECTIONS_Y: i32 = 6;
    const COLUMNS: i32 = 5;

    include!("incremental_terrain/cases.rs");
}
