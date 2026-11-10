//! The scene as console lines: the settings, the forceloaded areas, the pen and the pigs.

use glam::DVec3;

use crate::adapter::PlayerMode;

/// The block the pigs stand in on vanilla's default superflat (grass block at y = -61).
pub const FLOOR_Y: i32 = -60;
/// Vanilla's `forceload add` refuses an area of more than 256 chunks.
const FORCELOAD_SIDE_CHUNKS: i32 = 16;
/// Forceloaded beyond each area so that a pig at its edge still ticks.
const FORCELOAD_MARGIN: i32 = 16;
pub const SAMPLE_PREFIX: &str = "mbs";
pub const WATCHER_NAME: &str = "mobbench";

/// Whether the pigs of a population run their AI.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ai {
    On,
    /// `NoAI:1b`: no AI step, so no stroll and no change of the idle time.
    Off,
}

/// A square of blocks `min_x..min_x + size`, `min_z..min_z + size`.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize)]
pub struct Area {
    pub min_x: i32,
    pub min_z: i32,
    pub size: i32,
}

impl Area {
    #[must_use]
    pub fn center(self) -> DVec3 {
        DVec3::new(
            f64::from(self.min_x) + f64::from(self.size) / 2.0,
            f64::from(FLOOR_Y),
            f64::from(self.min_z) + f64::from(self.size) / 2.0,
        )
    }
}

/// The fenced pen: 250 × 250 blocks, so 50,000 pigs stand at 0.8 a block.
pub const PEN: Area = Area {
    min_x: -125,
    min_z: -125,
    size: 250,
};

/// The open area of the free roam phase: 640 × 640 blocks, 0.12 pigs a block at 50,000.
pub const ROAM: Area = Area {
    min_x: 380,
    min_z: -320,
    size: 640,
};

/// Where the watcher hovers in the crowd phases: outside the west fence, at the pen's middle.
pub const PEN_WATCH: DVec3 = DVec3::new(-130.5, -58.0, 0.5);

/// The game rules and world settings, equal on every server.
#[must_use]
pub fn settings() -> Vec<String> {
    [
        "gamerule spawn_mobs false",
        "gamerule spawn_monsters false",
        "gamerule spawn_patrols false",
        "gamerule spawn_wandering_traders false",
        "gamerule advance_time false",
        "gamerule advance_weather false",
        "gamerule max_entity_cramming 0",
        "gamerule mob_drops false",
        "time set noon",
        "weather clear",
    ]
    .map(str::to_owned)
    .to_vec()
}

/// `forceload add` lines that cover `area` and its margin, at most 256 chunks a line.
#[must_use]
pub fn forceload(area: Area) -> Vec<String> {
    let low = |min: i32| (min - FORCELOAD_MARGIN).div_euclid(16);
    let high = |min: i32| (min + area.size - 1 + FORCELOAD_MARGIN).div_euclid(16);
    let mut lines = Vec::new();
    let mut chunk_x = low(area.min_x);
    while chunk_x <= high(area.min_x) {
        let end_x = (chunk_x + FORCELOAD_SIDE_CHUNKS - 1).min(high(area.min_x));
        let mut chunk_z = low(area.min_z);
        while chunk_z <= high(area.min_z) {
            let end_z = (chunk_z + FORCELOAD_SIDE_CHUNKS - 1).min(high(area.min_z));
            lines.push(format!(
                "forceload add {} {} {} {}",
                chunk_x * 16,
                chunk_z * 16,
                end_x * 16 + 15,
                end_z * 16 + 15
            ));
            chunk_z = end_z + 1;
        }
        chunk_x = end_x + 1;
    }
    lines
}

/// Four `fill` lines: a ring of oak fence one block outside `area`.
#[must_use]
pub fn fence(area: Area) -> Vec<String> {
    let (low_x, low_z) = (area.min_x - 1, area.min_z - 1);
    let (high_x, high_z) = (area.min_x + area.size, area.min_z + area.size);
    let y = FLOOR_Y;
    [
        (low_x, low_z, high_x, low_z),
        (low_x, high_z, high_x, high_z),
        (low_x, low_z, low_x, high_z),
        (high_x, low_z, high_x, high_z),
    ]
    .map(|(x1, z1, x2, z2)| format!("fill {x1} {y} {z1} {x2} {y} {z2} minecraft:oak_fence"))
    .to_vec()
}

/// The first int of every pig's UUID (`mbs` in ASCII); the second is the generation of the
/// population, the last the pig's index. Vanilla refuses a UUID of a pig killed earlier in the run
/// (`Unable to summon entity due to duplicate UUIDs`), so each population gets new ones.
const UUID_HIGH: u32 = 0x006d_6273;
/// Steps through the pig indices so that one round of panic hits reaches pigs all over the area;
/// coprime with
/// any pig count that is not a multiple of 7919.
const STIR_STEP: usize = 7919;

/// The UUID of pig `index` of population `generation`, as the console writes it.
#[must_use]
pub fn pig_uuid(generation: u32, index: usize) -> String {
    format!(
        "{UUID_HIGH:08x}-{:04x}-{:04x}-0000-0000{:08x}",
        generation >> 16,
        generation & 0xffff,
        index as u32
    )
}

/// The pig index of named sample `sample`.
#[must_use]
pub fn sample_index(count: usize, samples: usize, sample: usize) -> usize {
    let stride = count
        .checked_div(samples)
        .map_or(usize::MAX, |stride| stride.max(1));
    sample * stride + stride / 2
}

/// One `summon` line a pig, on an even grid over `area`. Each pig gets a UUID from its index, so
/// the console can address it without an entity selector: a selector scans every entity, which
/// takes Horizon about 5 s at 50,000 pigs. The named samples also get the custom name `mbs<i>`,
/// so that `data get` answers name the pig they describe. A compound with a key skips the first
/// spawn setup, so every pig is an adult of the default variant (TRK-SUM-003); neither key changes
/// the idle time (GOAL-STR-004).
#[must_use]
pub fn summons(area: Area, generation: u32, count: usize, samples: usize, ai: Ai) -> Vec<String> {
    let side = (count as f64).sqrt().ceil().max(1.0) as usize;
    let spacing = f64::from(area.size) / side as f64;
    let named: Vec<usize> = (0..samples)
        .map(|sample| sample_index(count, samples, sample))
        .collect();
    (0..count)
        .map(|index| {
            let x = f64::from(area.min_x) + ((index % side) as f64 + 0.5) * spacing;
            let z = f64::from(area.min_z) + ((index / side) as f64 + 0.5) * spacing;
            let no_ai = match ai {
                Ai::On => "",
                Ai::Off => ",NoAI:1b",
            };
            let uuid = format!(
                "UUID:[I;{UUID_HIGH},{},0,{}]{no_ai}",
                generation as i32,
                index as u32 as i32
            );
            match named.iter().position(|named| *named == index) {
                Some(sample) => format!(
                    "summon minecraft:pig {x:.2} {FLOOR_Y} {z:.2} {{{uuid},CustomName:\"{SAMPLE_PREFIX}{sample}\"}}"
                ),
                None => format!("summon minecraft:pig {x:.2} {FLOOR_Y} {z:.2} {{{uuid}}}"),
            }
        })
        .collect()
}

pub const KILL_PIGS: &str = "kill @e[type=minecraft:pig]";
pub const COUNT_PIGS: &str = "execute if entity @e[type=minecraft:pig]";
pub const COUNT_SPECTATORS: &str = "execute if entity @a[gamemode=spectator]";

/// Panic round `round`: one line a pig for `hit` of the `count` pigs, 0 damage of type `cactus`
/// each. `cactus` is in `panic_causes`, so each hit pig panics, with no health loss and no
/// knockback. Panic does not spread; the neighbours move only when the panicking pigs push them.
/// Each round hits the next `hit` pigs of a fixed spread-out order.
#[must_use]
pub fn panic_hits(generation: u32, round: usize, hit: usize, count: usize) -> Vec<String> {
    (0..hit.min(count))
        .map(|offset| {
            let index = ((round * hit + offset) * STIR_STEP) % count.max(1);
            format!("damage {} 0 minecraft:cactus", pig_uuid(generation, index))
        })
        .collect()
}

#[must_use]
pub fn sample_query(generation: u32, count: usize, samples: usize, sample: usize) -> String {
    format!(
        "data get entity {} Pos",
        pig_uuid(generation, sample_index(count, samples, sample))
    )
}

#[must_use]
pub fn game_mode(player: &str, mode: PlayerMode) -> String {
    format!("gamemode {} {player}", mode.as_str())
}

#[must_use]
pub fn teleport(player: &str, at: DVec3) -> String {
    format!("tp {player} {} {} {}", at.x, at.y, at.z)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forceload_covers_the_area_and_its_margin_within_the_chunk_limit() {
        let lines = forceload(ROAM);
        let mut chunks = 0;
        for line in &lines {
            let numbers: Vec<i32> = line
                .trim_start_matches("forceload add ")
                .split(' ')
                .map(|part| part.parse().unwrap())
                .collect();
            let [x1, z1, x2, z2] = numbers[..] else {
                panic!("{line}");
            };
            let area = ((x2 - x1 + 1) / 16) * ((z2 - z1 + 1) / 16);
            assert!(area <= 256, "{line} names {area} chunks");
            chunks += area;
        }
        // x: 364 in chunk 22 to 1035 in chunk 64, 43 chunks; z: -336 in chunk -21 to 335 in
        // chunk 20, 42 chunks.
        assert_eq!(chunks, 43 * 42);
    }

    #[test]
    fn the_panic_rounds_hit_every_pig_once_before_any_twice() {
        let mut hit = std::collections::HashSet::new();
        for round in 0..25 {
            for line in panic_hits(1, round, 2000, 50_000) {
                assert!(
                    hit.insert(line),
                    "a pig was hit twice in the first 50,000 hits"
                );
            }
        }
    }

    #[test]
    fn the_summons_stay_in_the_area_and_keep_the_sample_and_uuid_formats() {
        let lines = summons(PEN, 1, 50_000, 32, Ai::On);
        assert_eq!(lines.len(), 50_000);
        let named = lines
            .iter()
            .filter(|line| line.contains("CustomName"))
            .count();
        assert_eq!(named, 32);
        for line in &lines {
            let parts: Vec<f64> = line
                .split(' ')
                .skip(2)
                .take(3)
                .map(|part| part.parse().unwrap())
                .collect();
            assert!((-125.0..125.0).contains(&parts[0]), "{line}");
            assert!((-125.0..125.0).contains(&parts[2]), "{line}");
        }
        // [I;7168627,2,0,1234]: the ints in order are the 32-bit groups of the hex form.
        assert_eq!(pig_uuid(2, 1234), "006d6273-0000-0002-0000-0000000004d2");
    }
}
