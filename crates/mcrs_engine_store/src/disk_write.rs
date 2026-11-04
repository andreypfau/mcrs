/// Whether a write to the disk succeeded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiskWrite {
    Done,
    Failed,
}
