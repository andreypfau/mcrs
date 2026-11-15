use mcrs_minecraft_resource::ResourceLocationBuf;

use crate::condition_problem::ConditionProblem;
use crate::member_kind::MemberKind;

/// A problem of one named entry that the load refuses (LOOT-CTX-016).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NamedIssue {
    pub entry: ResourceLocationBuf,
    pub kind: MemberKind,
    pub problem: ConditionProblem,
}
