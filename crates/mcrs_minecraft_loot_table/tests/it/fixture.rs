use std::iter;
use std::num::NonZeroU8;
use std::sync::LazyLock;

use glam::DVec3;
use mcrs_minecraft_block::definition::BlockDefinitions;
use mcrs_minecraft_engine_registries::{test_corpus, test_item_table};
use mcrs_minecraft_entity::Pose;
use mcrs_minecraft_entity::entity_type_keys::EntityType;
use mcrs_minecraft_entity::mob_effect_keys::MobEffect;
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
use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_nbt_path::NbtPath;
use mcrs_minecraft_predicate::{ActiveEffect, EntityView, ItemSubject, LocationView};
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

/// One `name` with the levels of `enchantments` in its component patch.
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

/// The context of a break of `state` at the origin by an entity with `tool`; `None` is the
/// empty hand.
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

/// The key of a survival break at the origin at `tick`.
pub fn keyed(table: Id<LootTable>, tick: u64) -> KeyedSeed {
    KeyedSeed {
        world: 7,
        table,
        origin: BlockPos::new(0, 64, 0),
        tick,
        cause: 1,
    }
}

/// The keyed stream of `table` in `loot` for a survival break at the origin at `tick`.
pub fn stream(loot: &LootTables, table: Id<LootTable>, tick: u64) -> XoroshiroRandom {
    loot.keyed_stream(keyed(table, tick))
        .expect("a baked table")
}

/// The stacks as item names and counts; the empty stack is `("", 0)`.
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

/// The blocks around the origin of a break, and the stack in the main hand of the entity of the
/// context. With a stack the entity is living and has no other equipment; without one it is not
/// living.
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

    fn is_loaded(&self, pos: BlockPos) -> bool {
        self.block_state(pos).is_some()
    }

    fn fluid_state(&self, _: BlockPos) -> Option<mcrs_minecraft_block::definition::FluidState> {
        None
    }

    fn block_light(&self, _: BlockPos) -> Option<u8> {
        None
    }

    fn sky_light(&self, _: BlockPos) -> Option<u8> {
        None
    }

    fn sky_light_level(&self) -> Option<f32> {
        None
    }

    fn biome(&self, _: BlockPos) -> Option<Id<mcrs_minecraft_biome::Biome>> {
        None
    }

    fn inside_structure(
        &self,
        _: BlockPos,
        _: Id<mcrs_minecraft_entity::spawn_condition::Structure>,
    ) -> Option<bool> {
        None
    }

    fn dimension(&self) -> Option<Id<mcrs_minecraft_dimension::Dimension>> {
        None
    }

    fn block_entity_data(
        &self,
        _: BlockPos,
        _: &mut mcrs_minecraft_nbt::compound::NbtCompound,
    ) -> bool {
        false
    }
}

impl EntityView for Around<'_> {
    type Entity = ();

    fn entity_type(&self, (): ()) -> Option<Id<EntityType>> {
        None
    }

    fn position(&self, (): ()) -> Option<DVec3> {
        None
    }

    fn is_on_ground(&self, (): ()) -> Option<bool> {
        None
    }

    fn support_position(&self, (): ()) -> Option<BlockPos> {
        None
    }

    fn movement_position(&self, (): ()) -> Option<BlockPos> {
        None
    }

    fn reported_movement(&self, (): ()) -> Option<DVec3> {
        None
    }

    fn fall_distance(&self, (): ()) -> Option<f64> {
        None
    }

    fn saved_data(&self, (): (), _: &mut NbtCompound) -> bool {
        false
    }

    fn is_burning(&self, (): ()) -> Option<bool> {
        None
    }

    fn pose(&self, (): ()) -> Option<Pose> {
        None
    }

    fn is_sprinting(&self, (): ()) -> Option<bool> {
        None
    }

    fn is_swimming(&self, (): ()) -> Option<bool> {
        None
    }

    fn is_in_water(&self, (): ()) -> Option<bool> {
        None
    }

    fn is_living(&self, (): ()) -> Option<bool> {
        self.main_hand.map(|_| true)
    }

    fn is_fall_flying(&self, (): ()) -> Option<bool> {
        None
    }

    fn has_flying_ability(&self, (): ()) -> Option<bool> {
        None
    }

    fn is_baby(&self, (): ()) -> Option<bool> {
        None
    }

    fn active_effect(&self, (): (), _: Id<MobEffect>) -> Option<ActiveEffect> {
        None
    }

    fn equipment(&self, (): (), slot: EquipmentSlot) -> Option<ItemSubject<'_>> {
        let stack = self.main_hand?;
        Some(match slot {
            EquipmentSlot::MainHand => ItemSubject::Stack(stack),
            _ => ItemSubject::Empty,
        })
    }

    fn slot(&self, (): (), _: u16) -> Option<ItemSubject<'_>> {
        None
    }

    fn tick_count(&self, (): ()) -> Option<u32> {
        None
    }

    fn vehicle(&self, (): ()) -> Option<()> {
        None
    }

    fn passengers(&self, (): ()) -> Option<impl Iterator<Item = ()>> {
        None::<iter::Empty<()>>
    }

    fn attack_target(&self, (): ()) -> Option<()> {
        None
    }

    fn team(&self, (): ()) -> Option<&str> {
        None
    }

    fn has_scoreboard_tag(&self, (): (), _: &str) -> Option<bool> {
        None
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

    fn thunder_strength(&self) -> Option<f32> {
        None
    }
}

impl ClockView for Around<'_> {
    fn total_ticks(&self, _: Id<WorldClock>) -> Option<i64> {
        None
    }
}
