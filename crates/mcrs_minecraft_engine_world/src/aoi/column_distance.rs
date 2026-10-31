use mcrs_minecraft_core::ColumnPos;

/// The squared distance in columns from `pos` to the nearest of `players`, or 0 without one.
#[must_use]
pub fn min_column_distance(pos: &ColumnPos, players: &[ColumnPos]) -> i32 {
    players
        .iter()
        .map(|player| pos.distance_squared(*player))
        .min()
        .unwrap_or(0)
}
