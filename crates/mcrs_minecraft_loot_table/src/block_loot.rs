use mcrs_minecraft_block::definition::{BlockDefinitions, LootId};
use mcrs_minecraft_item_key::item_keys::Item;
use mcrs_minecraft_item_stack::{Kind, MaxStackSize};
use mcrs_minecraft_item_table::ItemTable;
use mcrs_minecraft_registries::Registries;
use mcrs_minecraft_resource::Id;
use mcrs_voxel::{BlockPos, BlockStateId};

use crate::baker::Baker;
use crate::draws::Draws;
use crate::loot_context::LootContext;
use crate::loot_issue::{LootIssue, LootProblem};
use crate::loot_table_file::LootTableFile;
use crate::table::Table;

/// The block tables of a load, baked to ids, by the loot id of the block definitions, with the
/// maximum stack size of each item.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct BlockLoot {
    tables: Box<[Option<Table>]>,
    max_stack: Box<[u8]>,
}

/// One stack of a drop list, at most the maximum stack size of its item.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DroppedStack {
    pub item: Id<Item>,
    pub count: u8,
}

/// What the random values of one evaluation depend on besides the table and the origin block
/// (LOOT-RND-001).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LootSeed {
    pub world: u64,
    pub tick: u64,
    pub cause: LootCause,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LootCause {
    /// A survival break with a harvest.
    Break,
    /// A removal that drops, such as a block that lost its support.
    Removal,
}

impl BlockLoot {
    /// Bakes the table of each loot id of `definitions` from the loot tables of `registries`.
    pub fn bake(
        definitions: &BlockDefinitions,
        registries: &Registries,
        items: &ItemTable,
    ) -> (Self, Vec<LootIssue>) {
        let files = registries.registry::<LootTableFile>();
        let mut issues = Vec::new();
        let mut tables = Vec::with_capacity(definitions.loot_table_count());
        for number in 0..definitions.loot_table_count() {
            let Some(name) = u16::try_from(number)
                .ok()
                .and_then(|number| definitions.loot_table(LootId(number)))
            else {
                tables.push(None);
                continue;
            };
            let file = files.and_then(|files| files.get(files.by_name(name.as_location())?));
            let Some(file) = file else {
                issues.push(LootIssue {
                    table: name.clone(),
                    problem: LootProblem::MissingTable,
                });
                tables.push(None);
                continue;
            };
            let mut baker = Baker::new(definitions, registries);
            tables.push(Some(baker.table(file, key_of(name.as_str()))));
            issues.extend(baker.problems.into_iter().map(|problem| LootIssue {
                table: name.clone(),
                problem,
            }));
        }
        let item_count = registries.entries::<Item>().map_or(0, |items| items.len());
        let max_stack = (0..item_count)
            .map(|number| {
                u16::try_from(number)
                    .ok()
                    .and_then(|number| items.prototype(Kind(number)))
                    .and_then(|prototype| prototype.get::<MaxStackSize>())
                    .and_then(|size| u8::try_from(size.0).ok())
                    .unwrap_or(1)
                    .max(1)
            })
            .collect();
        let loot = Self {
            tables: tables.into_boxed_slice(),
            max_stack,
        };
        (loot, issues)
    }

    /// Adds to `out` the stacks of one evaluation of `table`, divided at the maximum stack size
    /// of each item (LOOT-OUT-001); a count of 0 or less makes nothing (LOOT-OUT-002).
    pub fn drops<W: Fn(BlockPos) -> Option<BlockStateId>>(
        &self,
        table: LootId,
        cx: &LootContext<'_, W>,
        seed: LootSeed,
        out: &mut Vec<DroppedStack>,
    ) {
        let Some(table) = self
            .tables
            .get(usize::from(table.0))
            .and_then(Option::as_ref)
        else {
            return;
        };
        let mut draws = Draws::new(stream_seed(table.key, cx.origin, seed));
        let mut counts = Vec::new();
        table.evaluate(cx, &mut draws, &mut counts);
        for (item, mut count) in counts {
            let max = self
                .max_stack
                .get(usize::from(item.number()))
                .copied()
                .unwrap_or(1);
            while count > 0 {
                let part = u8::try_from(count).unwrap_or(u8::MAX).min(max);
                out.push(DroppedStack { item, count: part });
                count -= i32::from(part);
            }
        }
    }
}

fn mix(value: u64) -> u64 {
    // SplitMix64 finaliser.
    let value = (value ^ (value >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    let value = (value ^ (value >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    value ^ (value >> 31)
}

fn key_of(name: &str) -> u64 {
    // FNV-1a.
    name.bytes().fold(0xCBF2_9CE4_8422_2325, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01B3)
    })
}

fn stream_seed(table: u64, origin: BlockPos, seed: LootSeed) -> u64 {
    let cause = match seed.cause {
        LootCause::Break => 1,
        LootCause::Removal => 2,
    };
    [
        table,
        u64::from(origin.x.cast_unsigned()),
        u64::from(origin.y.cast_unsigned()),
        u64::from(origin.z.cast_unsigned()),
        seed.tick,
        cause,
    ]
    .into_iter()
    .fold(mix(seed.world), |state, word| mix(state ^ word))
}
