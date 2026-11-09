use glam::DVec3;
use mcrs_minecraft_entity_key::EntityId;
use mcrs_minecraft_entity_key::EntityKey;

use crate::BlockAction;
use crate::GridFlags;
use crate::GridRow;
use crate::NoBodies;
use crate::PushTargets;
use crate::QueryEntity;
use crate::Scratch;
use crate::Tables;
use crate::TeamSlot;
use crate::TestBlocks;
use crate::TestWorld;
use crate::View;

/// The test blocks, their tables, an empty world and the buffers of a tick, for tests and
/// benchmarks.
pub struct TestFixture {
    pub blocks: TestBlocks,
    pub tables: Tables,
    pub world: TestWorld,
    pub scratch: Scratch,
    pub actions: Vec<BlockAction>,
}

impl TestFixture {
    #[must_use]
    pub fn new() -> Self {
        let blocks = TestBlocks::new();
        let tables = Tables::new(blocks.table.clone());
        Self {
            blocks,
            tables,
            world: TestWorld::new(),
            scratch: Scratch::new(),
            actions: Vec::new(),
        }
    }

    #[must_use]
    pub fn view(&self) -> View<'_, TestWorld, NoBodies> {
        View::new(&self.world, &NoBodies, &self.tables)
    }

    /// A query entity at `position` with no fall, shift or powder snow rule.
    #[must_use]
    pub fn query_entity(position: DVec3) -> QueryEntity {
        QueryEntity {
            key: EntityKey(0),
            position,
            fall_distance: 0.0,
            shift: false,
            powder_snow_solid: false,
            walks_on_powder_snow: false,
        }
    }

    /// The grid row of a pushable mob of width 0.6 and height 1.95 at `(x, 0, z)`, with the
    /// sequence `index` in its key.
    #[must_use]
    pub fn mob(index: u32, x: f64, z: f64) -> GridRow {
        let id = EntityId(u64::from(index) + 1);
        GridRow {
            position: DVec3::new(x, 0.0, z),
            half_width: 0.3,
            height: 1.95,
            key: EntityKey(u128::from(index)),
            id,
            root_vehicle: id,
            team: TeamSlot::default(),
            flags: GridFlags::PUSHABLE,
            push_targets: PushTargets::Each,
        }
    }
}

impl Default for TestFixture {
    fn default() -> Self {
        Self::new()
    }
}

/// A SplitMix64 stream, for random tests.
pub struct TestRng(pub u64);

impl TestRng {
    pub fn next_u64(&mut self) -> u64 {
        let value = split_mix(self.0);
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        value
    }

    pub fn range(&mut self, low: f64, high: f64) -> f64 {
        low + (high - low) * ((self.next_u64() >> 11) as f64 / (1_u64 << 53) as f64)
    }

    pub fn below(&mut self, count: usize) -> usize {
        (self.next_u64() % count as u64) as usize
    }

    pub fn chance(&mut self, numerator: u64, denominator: u64) -> bool {
        self.next_u64() % denominator < numerator
    }
}

fn split_mix(value: u64) -> u64 {
    let mut z = value.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}
