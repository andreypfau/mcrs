use mcrs_minecraft_loot_condition::MemberKind;

/// The kind of the named part that a [`LootIssue`](crate::LootIssue) names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum IssueOwner {
    /// An entry of the registry `loot_table`.
    Table,
    /// An entry of the registry `item_modifier`.
    ItemModifier,
    /// An entry of the registry `predicate`, `context_int_provider` or `context_float_provider`
    /// (LOOT-CTX-016).
    Named(MemberKind),
}
