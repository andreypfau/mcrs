use serde::Deserialize;

use crate::condition_file::ConditionFile;

/// One element of the `cases` of `number_dispatcher` (REG-FLD-731).
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct DispatchCaseFile<P> {
    pub(crate) condition: ConditionFile,
    pub(crate) value: P,
}
