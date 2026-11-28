use mcrs_voxel::ColumnPos;

#[must_use]
pub fn min_column_distance(pos: &ColumnPos, players: &[ColumnPos]) -> i32 {
    players
        .iter()
        .map(|player| pos.distance_squared(*player))
        .min()
        .unwrap_or(0)
}
