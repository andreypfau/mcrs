use serde::Deserialize;

use crate::condition_file::ConditionFile;

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct DispatchCaseFile<P> {
    pub(crate) condition: ConditionFile,
    pub(crate) value: P,
}
