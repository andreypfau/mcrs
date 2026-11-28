#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct StageRun {
    pub decisions: usize,
    pub evaluations: u64,
    pub mobs: usize,
}
