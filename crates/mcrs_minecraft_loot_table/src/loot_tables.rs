use mcrs_minecraft_block::definition::{BlockDefinitions, LootId};
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_item_table::ItemTable;
use mcrs_minecraft_loot::loot::LootTable;
use mcrs_minecraft_loot_condition::{
    ContextParam, ContextParams, LootContext, LootWorld, NamedEntries,
};
use mcrs_minecraft_random::Random;
use mcrs_minecraft_random::xoroshiro::XoroshiroRandom;
use mcrs_minecraft_registries::Registries;
use mcrs_minecraft_resource::Id;

use crate::baker;
use crate::issue_owner::IssueOwner;
use crate::item_rules::ItemRules;
use crate::keyed_seed::{self, KeyedSeed};
use crate::loot_issue::{LootIssue, LootProblem};
use crate::loot_scratch::LootScratch;
use crate::loot_table_file::LootTableFile;
use crate::roll_error::RollError;
use crate::table::{Evaluation, Table};

/// The loot tables of one registry load, baked to ids and addressed by their id in the registry
/// `loot_table`.
#[derive(Debug, Clone)]
pub struct LootTables {
    /// The tables of the registry at the index of their id, then the inline tables.
    tables: Box<[Table]>,
    named: usize,
    entries: NamedEntries,
    by_block: Box<[Option<Id<LootTable>>]>,
    rules: ItemRules,
}

impl LootTables {
    /// Bakes each named provider and each table of the registry `loot_table` of `registries`, maps
    /// each loot id of `definitions` to the table that it names, and takes the components of each
    /// item from `items` for the modifiers and the division (LOOT-OUT-001).
    #[must_use]
    pub fn bake(
        definitions: &BlockDefinitions,
        registries: &Registries,
        items: &ItemTable,
    ) -> (Self, Vec<LootIssue>) {
        let mut issues = Vec::new();
        let files = registries.registry::<LootTableFile>();
        let rules = ItemRules::bake(registries, items);
        let (entries, entry_issues) = NamedEntries::bake(definitions, registries, *items);
        issues.extend(entry_issues.into_iter().map(|issue| LootIssue {
            name: issue.entry,
            owner: IssueOwner::Named(issue.kind),
            path: String::new(),
            problem: LootProblem::Condition(issue.problem),
        }));
        let tables = baker::bake(definitions, registries, &entries, &mut issues).into_boxed_slice();
        let named = files.map_or(0, |files| files.len());
        let by_block = (0..definitions.loot_table_count())
            .map(|number| {
                let name = u16::try_from(number)
                    .ok()
                    .and_then(|number| definitions.loot_table(LootId(number)))?;
                let id = files.and_then(|files| files.by_name(name.as_location()));
                if id.is_none() {
                    issues.push(LootIssue {
                        name: name.clone(),
                        owner: IssueOwner::Table,
                        path: String::new(),
                        problem: LootProblem::MissingTable,
                    });
                }
                id
            })
            .collect();
        let loot = Self {
            tables,
            named,
            entries,
            by_block,
            rules,
        };
        (loot, issues)
    }

    /// The table that the block definitions name with `loot`; `None` for a block without a table
    /// and for a table that the load does not hold.
    #[must_use]
    pub fn block_table(&self, loot: LootId) -> Option<Id<LootTable>> {
        self.by_block.get(usize::from(loot.0)).copied().flatten()
    }

    /// The generator of the keyed stream of `seed` (LOOT-RND-001). It depends on the name of the
    /// table, not on its id, so a data pack that adds a table does not change it.
    pub fn keyed_stream(&self, seed: KeyedSeed) -> Result<XoroshiroRandom, RollError> {
        let baked = self.table(seed.table)?;
        Ok(keyed_seed::stream(seed, baked.name_key))
    }

    /// Adds the drop list of one evaluation of `table` to the end of `stacks`, with the draws
    /// from `random` (LOOT-EVL-001, LOOT-RND-008). Each stack is divided at the maximum stack
    /// size of its item; a stack with a count of 0 or less is `None`, the empty stack
    /// (LOOT-OUT-001). A nested table rolls with the same context, generator and buffers, and
    /// its stacks are divided only once they leave `table` (LOOT-EVL-010). A context that lacks
    /// a parameter that its key set requires, or holds one that it does not allow, is refused
    /// before any draw (LOOT-CTX-012). The type of the table has no part in this check.
    pub fn roll<E: Copy, W: LootWorld<E>, R: Random>(
        &self,
        table: Id<LootTable>,
        context: &LootContext<'_, E>,
        world: &W,
        random: &mut R,
        scratch: &mut LootScratch,
        stacks: &mut Vec<Option<ItemStack>>,
    ) -> Result<(), RollError> {
        let baked = self.table(table)?;
        check(context)?;
        let evaluation = Evaluation {
            tables: &self.tables,
            named: &self.entries,
            context,
            world,
            rules: &self.rules,
        };
        evaluation.root(baked, random, scratch, stacks);
        Ok(())
    }

    fn table(&self, table: Id<LootTable>) -> Result<&Table, RollError> {
        self.tables
            .get(table.index())
            .filter(|_| table.index() < self.named)
            .ok_or(RollError::UnknownTable(table))
    }
}

fn check<E>(context: &LootContext<'_, E>) -> Result<(), RollError> {
    let held = context.held();
    let required = ContextParams::required(context.key_set());
    let allowed = ContextParams::allowed(context.key_set());
    for param in ContextParam::ALL {
        if required.contains(param) && !held.contains(param) {
            return Err(RollError::MissingParam(param));
        }
        if held.contains(param) && !allowed.contains(param) {
            return Err(RollError::UnexpectedParam(param));
        }
    }
    Ok(())
}
