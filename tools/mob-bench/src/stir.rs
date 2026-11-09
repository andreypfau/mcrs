//! The stir: a hit of 0 `minecraft:drown` on every pig once each [`PERIOD`] game ticks, spread
//! evenly over those ticks. Only a non-spectator player within 32 blocks or a hit resets the idle
//! time of a pig, and the stroll goal is eligible only in the 99 AI steps after a reset
//! (`docs/spec/mob-goals.md`, GOAL-STR-004, GOAL-WAKE-020). The hit takes no health, gives no
//! knockback and no velocity, and does not make the pig panic.

use std::str::FromStr;

use anyhow::bail;
use serde::Serialize;

use crate::scene::{self, Area, FLOOR_Y};

/// Game ticks between two hits of one pig: below the 99 steps of the stroll window, with room
/// for a pig that walks into the next strip.
pub const PERIOD: i64 = 80;
const SLOTS: usize = PERIOD as usize;

/// How the console addresses the pigs of one tick of the stir.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    /// One `execute as @e[type=minecraft:pig,<box>]` line a tick, over a 1/80 strip of the area.
    Strip,
    /// One `damage <uuid>` line a pig, for the pigs whose index is the tick modulo 80.
    Uuid,
}

impl FromStr for Mode {
    type Err = anyhow::Error;

    fn from_str(name: &str) -> anyhow::Result<Self> {
        Ok(match name {
            "strip" => Mode::Strip,
            "uuid" => Mode::Uuid,
            other => bail!("unknown stir mode {other}"),
        })
    }
}

/// The stir of one pig population.
#[derive(Clone, Copy, Debug)]
pub struct Stir {
    pub mode: Mode,
    pub area: Area,
    /// Blocks beyond the area that the strips still cover, for pigs that strolled out of it.
    pub margin: i32,
    pub generation: u32,
    pub count: usize,
}

/// One strip: blocks x from `min_x` to `min_x + width`, z from `min_z` to `min_z + depth`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Strip {
    min_x: i32,
    width: i32,
    min_z: i32,
    depth: i32,
}

impl Stir {
    /// The lines of game tick `tick`.
    #[must_use]
    pub fn lines(&self, tick: i64) -> Vec<String> {
        let slot = tick.rem_euclid(PERIOD) as usize;
        match self.mode {
            Mode::Strip => {
                let strip = self.strip(slot);
                // A box option spans `d + 1` blocks (ARG-ENT-009).
                vec![format!(
                    "execute as @e[type=minecraft:pig,x={},y={},z={},dx={},dy=8,dz={}] run damage @s 0 minecraft:drown",
                    strip.min_x,
                    FLOOR_Y - 4,
                    strip.min_z,
                    strip.width - 1,
                    strip.depth - 1
                )]
            }
            Mode::Uuid => (slot..self.count)
                .step_by(SLOTS)
                .map(|index| {
                    format!(
                        "damage {} 0 minecraft:drown",
                        scene::pig_uuid(self.generation, index)
                    )
                })
                .collect(),
        }
    }

    /// Strips of a whole number of blocks: the span grows past `margin` to a multiple of 80.
    fn strip(&self, slot: usize) -> Strip {
        let least = self.area.size + 2 * self.margin;
        let width = (least + PERIOD as i32 - 1) / PERIOD as i32;
        let span = width * PERIOD as i32;
        let low_x = self.area.min_x - (span - self.area.size) / 2;
        let depth = self.area.size + 2 * self.margin;
        Strip {
            min_x: low_x + slot as i32 * width,
            width,
            min_z: self.area.min_z - self.margin,
            depth,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::{PEN, ROAM};

    #[test]
    fn the_strips_of_one_period_tile_the_area_and_its_margin_once() {
        for area in [PEN, ROAM] {
            let stir = Stir {
                mode: Mode::Strip,
                area,
                margin: 8,
                generation: 1,
                count: 50_000,
            };
            let strips: Vec<Strip> = (0..SLOTS).map(|slot| stir.strip(slot)).collect();
            for pair in strips.windows(2) {
                let [before, after] = pair else { continue };
                assert_eq!(before.min_x + before.width, after.min_x, "strips must abut");
            }
            let first = strips[0];
            let last = strips[SLOTS - 1];
            assert!(
                first.min_x <= area.min_x - 8,
                "the west margin must be covered"
            );
            assert!(
                last.min_x + last.width >= area.min_x + area.size + 8,
                "the east margin must be covered"
            );
            assert!(
                first.min_z <= area.min_z - 8,
                "the north margin must be covered"
            );
            assert!(
                first.min_z + first.depth >= area.min_z + area.size + 8,
                "the south margin must be covered"
            );
        }
    }

    #[test]
    fn the_uuid_lines_of_one_period_hit_every_pig_once() {
        let stir = Stir {
            mode: Mode::Uuid,
            area: PEN,
            margin: 0,
            generation: 3,
            count: 5_003,
        };
        let mut hit = std::collections::HashSet::new();
        for tick in 1000..1000 + PERIOD {
            for line in stir.lines(tick) {
                assert!(hit.insert(line), "a pig was hit twice in one period");
            }
        }
        assert_eq!(hit.len(), 5_003, "every pig must be hit in one period");
    }
}
