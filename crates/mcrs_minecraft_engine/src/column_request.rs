use mcrs_voxel::ColumnPos;

/// What the muxer asks of the version stream of a column of a dimension.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColumnRequest {
    /// Publish a version of the column once one is ready, and each change of the column from
    /// then on.
    Want(ColumnPos),
    /// No client holds the column or waits for it any more.
    Drop(ColumnPos),
}
