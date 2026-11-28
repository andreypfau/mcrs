use mcrs_voxel::ColumnPos;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColumnRequest {
    Want(ColumnPos),
    Drop(ColumnPos),
}
