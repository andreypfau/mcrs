use mcrs_minecraft_numeric as numeric;

/// The ticks that a head yaw packet takes to turn the head.
pub const HEAD_TURN_TICKS: u8 = 3;

/// The turn of the head towards the last head yaw that the server sent.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct HeadTurn {
    target: f32,
    ticks_left: u8,
}

impl HeadTurn {
    pub fn start(&mut self, target: f32) {
        self.target = target;
        self.ticks_left = HEAD_TURN_TICKS;
    }

    /// The head yaw after one more tick of the turn from `head_yaw`.
    pub fn step(&mut self, head_yaw: f32) -> f32 {
        if self.ticks_left == 0 {
            return head_yaw;
        }
        let next =
            head_yaw + numeric::wrap_degrees(self.target - head_yaw) / f32::from(self.ticks_left);
        self.ticks_left -= 1;
        next
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_head_reaches_the_target_in_three_equal_ticks() {
        let mut turn = HeadTurn::default();
        turn.start(-150.0);
        let mut head = 150.0;
        let mut seen = Vec::new();
        for _ in 0..4 {
            head = turn.step(head);
            seen.push(numeric::wrap_degrees(head));
        }
        let expected = [170.0, -170.0, -150.0, -150.0];
        for (seen, expected) in seen.iter().zip(expected) {
            assert!((seen - expected).abs() < 1e-4, "{seen} != {expected}");
        }
    }
}
