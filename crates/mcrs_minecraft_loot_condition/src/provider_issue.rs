use mcrs_minecraft_resource::ResourceLocationBuf;

use crate::condition_problem::ConditionProblem;

/// A problem that the bake found in an entry of the registry `context_int_provider` or
/// `context_float_provider`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderIssue {
    /// The key of the entry.
    pub provider: ResourceLocationBuf,
    pub problem: ConditionProblem,
}
