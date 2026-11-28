use std::time::Duration;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "bevy", derive(bevy_ecs::resource::Resource))]
pub struct PingLimits {
    pub max_unanswered: usize,
    pub timeout: Duration,
}

impl Default for PingLimits {
    fn default() -> Self {
        Self {
            max_unanswered: 2400,
            timeout: Duration::from_secs(60),
        }
    }
}
