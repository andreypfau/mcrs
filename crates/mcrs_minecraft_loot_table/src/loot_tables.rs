use mcrs_minecraft_block::definition::{BlockDefinitions, LootId};
use mcrs_minecraft_item_key::item_keys::Item;
use mcrs_minecraft_item_stack::{ItemStack, Kind, MaxStackSize};
use mcrs_minecraft_item_table::ItemTable;
use mcrs_minecraft_loot::loot::LootTable;
use mcrs_minecraft_loot_condition::{ContextParam, ContextParams, LootContext};
use mcrs_minecraft_predicate::LocationView;
use mcrs_minecraft_random::Random;
use mcrs_minecraft_random::xoroshiro::XoroshiroRandom;
use mcrs_minecraft_registries::Registries;
use mcrs_minecraft_resource::Id;

use crate::baker;
use crate::keyed_seed::{self, KeyedSeed};
use crate::loot_issue::{LootIssue, LootProblem};
use crate::loot_scratch::LootScratch;
use crate::loot_table_file::LootTableFile;
use crate::roll_error::RollError;
use crate::table::Table;

/// The loot tables of one registry load, baked to ids and addressed by their id in the registry
/// `loot_table`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct LootTables {
    tables: Box<[Table]>,
    by_block: Box<[Option<Id<LootTable>>]>,
    max_stack: Box<[u8]>,
}

impl LootTables {
    /// Bakes each table of the registry `loot_table` of `registries`, maps each loot id of
    /// `definitions` to the table that it names, and takes the maximum stack size of each item
    /// from `items` for the division (LOOT-OUT-001).
    #[must_use]
    pub fn bake(
        definitions: &BlockDefinitions,
        registries: &Registries,
        items: &ItemTable,
    ) -> (Self, Vec<LootIssue>) {
        let mut issues = Vec::new();
        let files = registries.registry::<LootTableFile>();
        let tables = files
            .into_iter()
            .flat_map(|files| files.iter())
            .map(|(_, name, file)| baker::bake(definitions, registries, name, file, &mut issues))
            .collect();
        let by_block = (0..definitions.loot_table_count())
            .map(|number| {
                let name = u16::try_from(number)
                    .ok()
                    .and_then(|number| definitions.loot_table(LootId(number)))?;
                let id = files.and_then(|files| files.by_name(name.as_location()));
                if id.is_none() {
                    issues.push(LootIssue {
                        table: name.clone(),
                        path: String::new(),
                        problem: LootProblem::MissingTable,
                    });
                }
                id
            })
            .collect();
        let item_count = registries.entries::<Item>().map_or(0, |items| items.len());
        let max_stack = (0..item_count)
            .map(|number| {
                u16::try_from(number)
                    .ok()
                    .and_then(|number| items.prototype(Kind(number)))
                    .and_then(|prototype| prototype.get::<MaxStackSize>())
                    .and_then(|size| u8::try_from(size.0).ok())
                    .unwrap_or(1)
            })
            .collect();
        let loot = Self {
            tables,
            by_block,
            max_stack,
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
    /// (LOOT-OUT-001). A context that lacks a parameter that its key set requires, or holds one
    /// that it does not allow, is refused before any draw (LOOT-CTX-012). The type of the table
    /// has no part in this check.
    pub fn roll<E, W: LocationView, R: Random>(
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
        baked.evaluate(context, world, &self.max_stack, random, scratch, stacks);
        Ok(())
    }

    fn table(&self, table: Id<LootTable>) -> Result<&Table, RollError> {
        self.tables
            .get(table.index())
            .ok_or(RollError::UnknownTable(table))
    }
}

fn check<E>(context: &LootContext<'_, E>) -> Result<(), RollError> {
    let held = held(context);
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

fn held<E>(context: &LootContext<'_, E>) -> ContextParams {
    [
        (context.origin.is_some(), ContextParam::Origin),
        (context.this_entity.is_some(), ContextParam::ThisEntity),
        (context.tool.is_some(), ContextParam::Tool),
        (context.block_state.is_some(), ContextParam::BlockState),
        (
            context.explosion_radius.is_some(),
            ContextParam::ExplosionRadius,
        ),
    ]
    .into_iter()
    .filter(|&(present, _)| present)
    .fold(ContextParams::EMPTY, |held, (_, param)| held.with(param))
}
