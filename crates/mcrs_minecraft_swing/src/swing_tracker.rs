use crate::SwingModifiers;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ActiveSwing {
    started_at: u64,
    scaled_duration: i32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "bevy", derive(bevy_ecs::component::Component))]
pub struct SwingTracker {
    last: Option<ActiveSwing>,
}

impl SwingTracker {
    #[must_use]
    pub fn can_start(&self, now: u64) -> bool {
        let Some(last) = self.last else {
            return true;
        };
        if now <= last.started_at {
            return false;
        }
        let half = u64::try_from(last.scaled_duration / 2).unwrap_or(0);
        now > last.started_at.saturating_add(half)
    }

    pub fn try_start(
        &mut self,
        now: u64,
        base_duration: i32,
        modifiers: SwingModifiers,
    ) -> Option<i32> {
        if !self.can_start(now) {
            return None;
        }
        let scaled_duration = modifiers.effective_duration(base_duration);
        self.last = Some(ActiveSwing {
            started_at: now,
            scaled_duration,
        });
        Some(scaled_duration)
    }
}
