//! Moving many brains at once leaves each tile as moving them one at a time does.

use mcrs_minecraft_brain::Cadence;
use mcrs_minecraft_brain::Stage;
use mcrs_minecraft_brain::StageConfig;
use mcrs_minecraft_brain::WorldSeed;
use mcrs_minecraft_entity_key::EntityId;
use mcrs_minecraft_entity_key::EntityKey;
use mcrs_minecraft_entity_key::TileKey;

type Snapshot = Vec<(TileKey, Vec<(EntityId, EntityKey, u64, u32)>)>;

fn snapshot(stage: &Stage<u32>) -> Snapshot {
    stage
        .tiles()
        .map(|(tile, brains)| {
            let entries = brains
                .iter()
                .map(|entry| (entry.mob, entry.key, entry.wake, *entry.brain))
                .collect();
            (tile, entries)
        })
        .collect()
}

#[test]
fn relocating_brains_at_once_equals_relocating_them_one_by_one() {
    let mut seed = 0x2545_F491_4F6C_DD1D_u64;
    let mut next = |bound: u64| {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed % bound
    };
    let mut stage = Stage::new(StageConfig::new(WorldSeed(1)));
    for mob in 0..600_u64 {
        let tile = TileKey::new(next(4) as i32, 0, next(4) as i32);
        let key = EntityKey(u128::from(next(1 << 40)) << 20 | u128::from(mob));
        stage.spawn(
            tile,
            EntityId(mob),
            key,
            Cadence::new(0, (mob % 4) as u8),
            mob as u32,
        );
    }
    let mut single = stage.clone();
    for _ in 0..5 {
        let moves: Vec<(EntityId, TileKey)> = (0..150)
            .map(|_| {
                (
                    EntityId(next(650)),
                    TileKey::new(next(5) as i32, 0, next(4) as i32),
                )
            })
            .filter(|(mob, _)| *mob != EntityId(649))
            .collect();
        let mut seen = std::collections::BTreeSet::new();
        let moves: Vec<_> = moves
            .into_iter()
            .filter(|(mob, _)| seen.insert(*mob))
            .collect();
        for &(mob, to) in &moves {
            single.relocate(mob, to);
        }
        stage.relocate_all(moves);
        assert_eq!(snapshot(&stage), snapshot(&single));
    }
}
