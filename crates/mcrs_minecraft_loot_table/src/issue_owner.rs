use mcrs_minecraft_loot_condition::MemberKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum IssueOwner {
    Table,
    ItemModifier,
    Named(MemberKind),
}
