#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SwingModifiers {
    pub haste: Option<u8>,
    pub conduit_power: Option<u8>,
    pub mining_fatigue: Option<u8>,
}

impl SwingModifiers {
    pub fn record_effect(&mut self, id: &str, amplifier: u8) {
        let path = id.strip_prefix("minecraft:").unwrap_or(id);
        match path {
            "haste" => self.haste = self.haste.max(Some(amplifier)),
            "conduit_power" => self.conduit_power = self.conduit_power.max(Some(amplifier)),
            "mining_fatigue" => self.mining_fatigue = self.mining_fatigue.max(Some(amplifier)),
            _ => {}
        }
    }

    #[must_use]
    pub fn from_effects<'a>(effects: impl IntoIterator<Item = (&'a str, u8)>) -> Self {
        let mut out = Self::default();
        for (id, amplifier) in effects {
            out.record_effect(id, amplifier);
        }
        out
    }

    #[must_use]
    pub fn speed_amplifier(self) -> Option<u8> {
        self.haste.max(self.conduit_power)
    }

    #[must_use]
    pub fn effective_duration(self, base_duration: i32) -> i32 {
        let base = base_duration.max(0);
        if let Some(amplifier) = self.speed_amplifier() {
            base.saturating_sub(1 + i32::from(amplifier)).max(0)
        } else if let Some(amplifier) = self.mining_fatigue {
            base.saturating_add((1 + i32::from(amplifier)).saturating_mul(2))
                .max(0)
        } else {
            base
        }
    }
}
