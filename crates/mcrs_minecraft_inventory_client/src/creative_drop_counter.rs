use bevy_ecs::component::Component;
use bevy_ecs::system::Query;

const DROP_COST: u16 = 20;
const DROP_LIMIT: u16 = 1280;

#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct CreativeDropCounter(u16);

impl CreativeDropCounter {
    pub(crate) fn allows_drop(self) -> bool {
        self.0 < DROP_LIMIT
    }

    pub(crate) fn add_drops(&mut self, drops: usize) {
        let drops = u16::try_from(drops).unwrap_or(u16::MAX);
        self.0 = self.0.saturating_add(DROP_COST.saturating_mul(drops));
    }
}

pub(crate) fn creative_drops_pending(counters: Query<&CreativeDropCounter>) -> bool {
    counters.iter().any(|counter| counter.0 > 0)
}

pub(crate) fn count_down_creative_drops(mut counters: Query<&mut CreativeDropCounter>) {
    for mut counter in &mut counters {
        if counter.0 != 0 {
            counter.0 = counter.0.saturating_sub(1);
        }
    }
}
