use std::time::Duration;

use bevy_ecs::component::Component;
use bevy_ecs::resource::Resource;
use mcrs_minecraft_text::Text;

use crate::Instant;

#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct InboundRateLimit(pub u32);

impl Default for InboundRateLimit {
    fn default() -> Self {
        Self(500)
    }
}

impl InboundRateLimit {
    #[must_use]
    pub fn reason() -> Text {
        Text::translate("disconnect.exceeded_packet_rate", [])
    }
}

const SECOND: Duration = Duration::from_secs(1);
const KEEP: f32 = 0.75;

#[derive(Component, Clone, Debug, Default)]
pub struct InboundPacketRate {
    second_start: Option<Instant>,
    count: u32,
    average: f32,
}

impl InboundPacketRate {
    pub fn count(&mut self, arrival: Instant, limit: InboundRateLimit) -> bool {
        if limit.0 == 0 {
            return true;
        }
        let start = *self.second_start.get_or_insert(arrival);
        let mut within_limit = true;
        let elapsed = arrival.saturating_duration_since(start);
        if elapsed >= SECOND {
            self.average = self.count as f32 + KEEP * (self.average - self.count as f32);
            within_limit = self.average <= limit.0 as f32;
            let empty_seconds = elapsed.as_secs().saturating_sub(1);
            self.average *= KEEP.powi(i32::try_from(empty_seconds).unwrap_or(i32::MAX));
            self.count = 0;
            let whole_seconds = Duration::from_secs(elapsed.as_secs());
            self.second_start = Some(start.checked_add(whole_seconds).unwrap_or(arrival));
        }
        self.count = self.count.saturating_add(1);
        within_limit
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn first_refused(per_second: &[u32], limit: u32) -> Option<usize> {
        let mut rate = InboundPacketRate::default();
        let start = Instant::now();
        for (second, &count) in per_second.iter().enumerate() {
            let second_start = start + SECOND * u32::try_from(second).unwrap();
            for packet in 0..count {
                if !rate.count(
                    second_start + SECOND * packet / count,
                    InboundRateLimit(limit),
                ) {
                    return Some(second);
                }
            }
        }
        None
    }

    #[test]
    fn lim_pkt_001_the_average_of_a_finished_second_ends_the_connection_past_the_limit() {
        let cases: [(&str, Vec<u32>, u32, Option<usize>); 5] = [
            ("P-1", vec![100; 10], 50, Some(3)),
            ("P-2", vec![50; 20], 50, None),
            ("P-3", vec![200, 1], 50, None),
            ("P-4", vec![201, 1], 50, Some(1)),
            ("P-5", vec![10_000, 1], 0, None),
        ];
        for (row, per_second, limit, refused) in cases {
            assert_eq!(first_refused(&per_second, limit), refused, "{row}");
        }
    }
}
