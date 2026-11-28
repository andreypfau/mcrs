const TICKS: u32 = 60;
const FADE: f32 = 20.0;

#[derive(Clone, Debug)]
pub struct Overlay<M> {
    entry: Option<(M, u64)>,
}

impl<M> Default for Overlay<M> {
    fn default() -> Self {
        Self { entry: None }
    }
}

impl<M> Overlay<M> {
    pub fn set(&mut self, message: M, now: u64) {
        self.entry = Some((message, now));
    }

    #[must_use]
    pub fn fades_at(&self, now: u64) -> bool {
        let alpha = |at: u64| self.shown(at, 0.0).map(|(_, alpha)| alpha);
        alpha(now) != alpha(now.saturating_sub(1))
    }

    pub fn shown(&self, now: u64, partial_tick: f32) -> Option<(&M, u8)> {
        let (message, set_at) = self.entry.as_ref()?;
        let age = now.saturating_sub(*set_at);
        let time_left = u64::from(TICKS).saturating_sub(age);
        let t = time_left as f32 - partial_tick;
        let alpha = (t * 255.0 / FADE).floor().min(255.0);
        if alpha <= 0.0 {
            return None;
        }
        Some((message, alpha as u8))
    }
}
