use bevy_ecs::component::Component;

const DROP_COST: u16 = 20;
const DROP_LIMIT: u16 = 1480;

#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct CreativeDrops {
    counter: u16,
    at: u64,
}

impl CreativeDrops {
    pub(crate) fn try_drop(&mut self, now: u64) -> bool {
        let elapsed = u16::try_from(now.saturating_sub(self.at)).unwrap_or(u16::MAX);
        let counter = self.counter.saturating_sub(elapsed);
        if counter >= DROP_LIMIT {
            return false;
        }
        self.counter = counter.saturating_add(DROP_COST);
        self.at = now;
        true
    }
}
