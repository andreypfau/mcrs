use std::time::Duration;

use mcrs_minecraft_brain::StageRun;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct StageStats {
    pub run: StageRun,
    pub elapsed: Duration,
}
