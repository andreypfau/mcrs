const NO_HIT: u16 = u16::MAX;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "bevy", derive(bevy_ecs::component::Component))]
pub struct HitRecord {
    last_tick: u32,
    last_damage_type: u16,
    strikes: u16,
}

const _: () = assert!(size_of::<HitRecord>() <= 8);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LastHit {
    pub tick: u32,
    pub damage_type: u16,
}

impl Default for HitRecord {
    fn default() -> Self {
        Self {
            last_tick: 0,
            last_damage_type: NO_HIT,
            strikes: 0,
        }
    }
}

impl HitRecord {
    #[must_use]
    pub fn last(&self) -> Option<LastHit> {
        (self.last_damage_type != NO_HIT).then_some(LastHit {
            tick: self.last_tick,
            damage_type: self.last_damage_type,
        })
    }

    #[must_use]
    pub fn strikes(&self) -> u16 {
        self.strikes
    }

    pub fn strike(&mut self) {
        self.strikes = self.strikes.wrapping_add(1);
    }

    pub fn accept(&mut self, tick: u64, damage_type: u32) {
        self.last_tick = tick as u32;
        self.last_damage_type = u16::try_from(damage_type)
            .unwrap_or(NO_HIT - 1)
            .min(NO_HIT - 1);
    }
}
