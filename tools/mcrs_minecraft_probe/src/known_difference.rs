use crate::StepTrace;
use serde::{Deserialize, Serialize};

/// One step where a server other than the reference gives a result that is not in the recorded
/// trace. That server must give exactly `step` for the step with the same id.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct KnownDifference {
    #[serde(flatten)]
    pub step: StepTrace,
    /// One sentence that states what differs and why it is open.
    pub reason: String,
}
