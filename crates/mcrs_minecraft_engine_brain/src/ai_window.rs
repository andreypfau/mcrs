use std::mem;
use std::time::Duration;

use crate::AiStats;

const LOG_PERIOD: u64 = 200;

#[derive(Debug, Default)]
pub(crate) struct AiWindow {
    ticks: u64,
    prepare: Duration,
    prepare_max: Duration,
    life_cycle: Duration,
    tile_tasks: Duration,
    publish: Duration,
    stage: Duration,
    stage_max: Duration,
    evaluations: u64,
    decisions: u64,
}

impl AiWindow {
    pub(crate) fn add(&mut self, tick: u64, stats: &AiStats) {
        self.ticks = self.ticks.saturating_add(1);
        self.prepare = self.prepare.saturating_add(stats.prepare);
        self.prepare_max = self.prepare_max.max(stats.prepare);
        self.life_cycle = self.life_cycle.saturating_add(stats.life_cycle);
        self.tile_tasks = self.tile_tasks.saturating_add(stats.tile_tasks);
        self.publish = self.publish.saturating_add(stats.publish);
        self.stage = self.stage.saturating_add(stats.stage);
        self.stage_max = self.stage_max.max(stats.stage);
        self.evaluations = self.evaluations.saturating_add(stats.evaluations);
        self.decisions = self
            .decisions
            .saturating_add(u64::try_from(stats.decisions).unwrap_or(u64::MAX));
        if !tick.is_multiple_of(LOG_PERIOD) {
            return;
        }
        let window = mem::take(self);
        if stats.mobs > 0 && window.ticks > 0 {
            tracing::debug!(
                target: "mcrs_ai",
                tick,
                mobs = stats.mobs,
                prepare_mean_us = window.prepare.as_micros() / u128::from(window.ticks),
                prepare_max_us = window.prepare_max.as_micros(),
                life_cycle_mean_us = window.life_cycle.as_micros() / u128::from(window.ticks),
                tile_tasks_mean_us = window.tile_tasks.as_micros() / u128::from(window.ticks),
                publish_mean_us = window.publish.as_micros() / u128::from(window.ticks),
                stage_mean_us = window.stage.as_micros() / u128::from(window.ticks),
                stage_max_us = window.stage_max.as_micros(),
                evaluations_per_tick = window.evaluations / window.ticks,
                decisions_per_tick = window.decisions / window.ticks,
                waits = stats.waits,
                "mob AI"
            );
        }
    }
}
