use std::time::Duration;

use bevy_ecs::component::Component;
use bevy_ecs::resource::Resource;
use mcrs_minecraft_text::Text;

use crate::Instant;

/// The `rate-limit` property: the largest average of packets per second of one connection. 0
/// turns the limit off, which is the default, so that no burst of a client ends its connection.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct InboundRateLimit(pub u32);

impl InboundRateLimit {
    /// The reason of the disconnect.
    #[must_use]
    pub fn reason() -> Text {
        Text::translate("disconnect.exceeded_packet_rate", [])
    }
}

const SECOND: Duration = Duration::from_secs(1);
/// Each second keeps this share of the average and takes the rest from its own count.
const KEEP: f32 = 0.75;

/// The packets of a connection in each second, and their running average.
///
/// The seconds follow the arrival times of the packets, not the server ticks: a server that
/// stalls reads a backlog in one go, and that burst is the server's, not the client's.
#[derive(Component, Clone, Debug, Default)]
pub struct InboundPacketRate {
    second_start: Option<Instant>,
    count: u32,
    average: f32,
}

impl InboundPacketRate {
    /// Counts a packet that arrived at `arrival`. Answers `false` when the average of a finished
    /// second is more than `limit`.
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

    /// Counts `per_second` packets in each of `seconds` seconds from `start`, and answers the
    /// second in which the rate first ends the connection.
    fn first_refused(
        rate: &mut InboundPacketRate,
        start: Instant,
        seconds: u32,
        per_second: u32,
        limit: u32,
    ) -> Option<u32> {
        for second in 0..seconds {
            for packet in 0..per_second {
                let arrival = start + SECOND * second + SECOND * packet / per_second;
                if !rate.count(arrival, InboundRateLimit(limit)) {
                    return Some(second);
                }
            }
        }
        None
    }

    #[test]
    fn a_steady_rate_above_the_limit_ends_the_connection_in_the_fourth_second() {
        let mut rate = InboundPacketRate::default();
        assert_eq!(
            first_refused(&mut rate, Instant::now(), 10, 100, 50),
            Some(3),
            "MVC-VAN-019: the averages are 25, 43.75 and 57.8 after the first three seconds"
        );
    }
}
