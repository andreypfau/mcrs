const TICKS: u32 = 60;
const FADE: f32 = 20.0;

/// The text above the hotbar (CHAT-RECV-005).
#[derive(Clone, Debug)]
pub struct Overlay<M> {
    message: Option<M>,
    time_left: u32,
}

impl<M> Default for Overlay<M> {
    fn default() -> Self {
        Self {
            message: None,
            time_left: 0,
        }
    }
}

impl<M> Overlay<M> {
    /// Shows `message` for the full time.
    pub fn set(&mut self, message: M) {
        self.message = Some(message);
        self.time_left = TICKS;
    }

    /// Counts one GUI tick of the time left.
    pub fn tick(&mut self) {
        self.time_left = self.time_left.saturating_sub(1);
    }

    /// The message and its alpha from 0 to 255, while it shows.
    pub fn shown(&self, partial_tick: f32) -> Option<(&M, u8)> {
        let t = self.time_left as f32 - partial_tick;
        let alpha = (t * 255.0 / FADE).floor().min(255.0);
        if alpha <= 0.0 {
            return None;
        }
        Some((self.message.as_ref()?, alpha as u8))
    }
}
