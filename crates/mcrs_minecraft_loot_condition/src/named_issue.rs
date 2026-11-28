use mcrs_minecraft_resource::ResourceLocationBuf;

use crate::condition_problem::ConditionProblem;
use crate::member_kind::MemberKind;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NamedIssue {
    pub entry: ResourceLocationBuf,
    pub kind: MemberKind,
    pub problem: ConditionProblem,
}
