use mcrs_minecraft_core::Direction;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Click {
    pub face: Direction,
    pub cursor_y: f32,
    pub yaw: f32,
    pub pitch: f32,
}

impl Click {
    pub(crate) fn horizontal_look(&self) -> Direction {
        Direction::from_y_rot(self.yaw)
    }

    pub(crate) fn look(&self) -> Direction {
        if self.pitch < -45.0 {
            Direction::Up
        } else if self.pitch > 45.0 {
            Direction::Down
        } else {
            self.horizontal_look()
        }
    }

    pub(crate) fn upper_half(&self) -> bool {
        match self.face {
            Direction::Up => false,
            Direction::Down => true,
            _ => self.cursor_y > 0.5,
        }
    }
}
