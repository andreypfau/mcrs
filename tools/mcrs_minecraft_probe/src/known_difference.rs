use crate::StepTrace;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct KnownDifference {
    #[serde(flatten)]
    pub step: StepTrace,
    pub reason: String,
}
