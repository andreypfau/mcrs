#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StageRun {
    pub decisions: usize,
    pub evaluations: u64,
}
