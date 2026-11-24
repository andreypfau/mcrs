use std::num::NonZeroU8;
use std::sync::LazyLock;

use glam::DVec3;
use mcrs_minecraft_block::definition::BlockDefinitions;
use mcrs_minecraft_engine_registries::{test_corpus, test_item_table};
use mcrs_minecraft_environment::environment_attribute_keys::EnvironmentAttribute;
use mcrs_minecraft_environment::{AttributeValue, WorldClock};
use mcrs_minecraft_game_registries::registry_load::test_registries;
use mcrs_minecraft_item::component::equippable::EquipmentSlot;
use mcrs_minecraft_item::enchantment::Enchantment;
use mcrs_minecraft_item_key::item_keys::Item;
use mcrs_minecraft_item_stack::{
    Count, EnchantmentId, EnchantmentLevel, Enchantments, ItemStack, Kind,
};
use mcrs_minecraft_loot::context_key_set_keys::ContextKeySet;
use mcrs_minecraft_loot::loot::LootTable;
use mcrs_minecraft_loot_condition::{
    ClockView, EnvironmentView, LootContext, NbtNumber, ScoreHolder, ScoreView, StorageView, Tool,
    WeatherView,
};
use mcrs_minecraft_loot_table::{KeyedSeed, LootIssue, LootTables};
use mcrs_minecraft_nbt_path::NbtPath;
use mcrs_minecraft_predicate::{EntityView, ItemSubject, LocationView};
use mcrs_minecraft_random::xoroshiro::XoroshiroRandom;
use mcrs_minecraft_resource::{Id, ResourceLocation};
use mcrs_voxel::{BlockPos, BlockStateId};

pub static LOOT: LazyLock<(LootTables, Vec<LootIssue>)> =
    LazyLock::new(|| LootTables::bake(blocks(), test_registries(), &test_item_table()));

const ORIGIN: DVec3 = DVec3::new(0.5, 64.5, 0.5);

pub fn blocks() -> &'static BlockDefinitions {
    &test_corpus().0.0
}

pub fn state(block: &str, values: &[(&str, &str)]) -> BlockStateId {
    let entry = blocks().block(block).expect("a vanilla block");
    values
        .iter()
        .fold(entry.default_state_id, |state, (name, value)| {
            entry
                .with_text(state, name, value)
                .expect("a property value")
        })
}

pub fn item(name: &str) -> Kind {
    let id = test_registries()
        .entries::<Item>()
        .and_then(|items| items.by_name(ResourceLocation::read(name).ok()?))
        .expect("a vanilla item");
    Kind(id.number())
}

fn enchantment(name: &str) -> EnchantmentId {
    let id: Id<Enchantment> = test_registries()
        .entries::<Enchantment>()
        .and_then(|enchantments| enchantments.by_name(ResourceLocation::read(name).ok()?))
        .expect("a vanilla enchantment");
    EnchantmentId(id.number())
}

pub fn tool(name: &str, enchantments: &[(&str, u8)]) -> ItemStack {
    let stack = ItemStack::new(item(name), Count::new(1).expect("a count of 1"));
    if enchantments.is_empty() {
        return stack;
    }
    let levels = enchantments.iter().map(|&(name, level)| EnchantmentLevel {
        enchantment: enchantment(name),
        level: NonZeroU8::new(level).expect("a level above 0"),
    });
    stack.set(Enchantments::new(levels).expect("distinct enchantments"))
}

pub fn context(state: BlockStateId, tool: Option<&ItemStack>) -> LootContext<'_, ()> {
    let mut context = LootContext::try_new(ContextKeySet::Block).expect("a source key set");
    context.origin = Some(ORIGIN);
    context.block_state = Some(state);
    context.tool = Some(tool.map_or(Tool::Empty, Tool::Stack));
    context.this_entity = Some(());
    context
}

pub fn block_table(loot: &LootTables, state: BlockStateId) -> Id<LootTable> {
    let loot_id = blocks().state(state).loot.expect("a block with a table");
    loot.block_table(loot_id).expect("a loaded table")
}

pub fn keyed(table: Id<LootTable>, tick: u64) -> KeyedSeed {
    KeyedSeed {
        world: 7,
        table,
        origin: BlockPos::new(0, 64, 0),
        tick,
        cause: 1,
    }
}

pub fn stream(loot: &LootTables, table: Id<LootTable>, tick: u64) -> XoroshiroRandom {
    loot.keyed_stream(keyed(table, tick))
        .expect("a baked table")
}

pub fn names(stacks: impl IntoIterator<Item = Option<ItemStack>>) -> Vec<(String, u8)> {
    let items = test_registries().entries::<Item>().expect("items");
    stacks
        .into_iter()
        .map(|stack| {
            let Some(stack) = stack else {
                return (String::new(), 0);
            };
            let id = Id::<Item>::checked(stack.kind.0, items.len()).expect("an item id");
            let name = items.name(id).expect("a named item");
            (name.as_str().to_owned(), stack.count.get())
        })
        .collect()
}

pub struct Around<'a> {
    pub blocks: &'a [(BlockPos, BlockStateId)],
    pub main_hand: Option<&'a ItemStack>,
}

impl LocationView for Around<'_> {
    fn block_state(&self, pos: BlockPos) -> Option<BlockStateId> {
        self.blocks
            .iter()
            .find(|(at, _)| *at == pos)
            .map(|&(_, state)| state)
    }
}

impl EntityView for Around<'_> {
    type Entity = ();

    fn is_living(&self, (): ()) -> Option<bool> {
        self.main_hand.map(|_| true)
    }

    fn equipment(&self, (): (), slot: EquipmentSlot) -> Option<ItemSubject<'_>> {
        let stack = self.main_hand?;
        Some(match slot {
            EquipmentSlot::MainHand => ItemSubject::Stack(stack),
            _ => ItemSubject::Empty,
        })
    }
}

impl<E> ScoreView<E> for Around<'_> {
    fn score(&self, _: ScoreHolder<'_, E>, _: &str) -> Option<i32> {
        None
    }
}

impl StorageView for Around<'_> {
    fn storage_number(&self, _: ResourceLocation<'_>, _: &NbtPath) -> Option<NbtNumber> {
        None
    }
}

impl EnvironmentView for Around<'_> {
    fn environment_value(
        &self,
        _: EnvironmentAttribute,
        _: Option<DVec3>,
    ) -> Option<AttributeValue> {
        None
    }
}

impl WeatherView for Around<'_> {
    fn rain_strength(&self) -> Option<f32> {
        None
    }
}

impl ClockView for Around<'_> {
    fn total_ticks(&self, _: Id<WorldClock>) -> Option<i64> {
        None
    }
}
