/// The damage type index that marks a record with no accepted hit.
const NO_HIT: u16 = u16::MAX;

/// The hits on a living entity that its AI reads (DMG-HIT-002 step 1, `mob-goals.md`
/// GOAL-PNC-001 and GOAL-STR-004). The damage procedure writes it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HitRecord {
    last_tick: u32,
    last_damage_type: u16,
    strikes: u16,
}

const _: () = assert!(size_of::<HitRecord>() <= 8);

/// An accepted hit: the low 32 bits of its tick and the network id of its damage type.
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
    /// The last hit that the damage cooldown accepted.
    #[must_use]
    pub fn last(&self) -> Option<LastHit> {
        (self.last_damage_type != NO_HIT).then_some(LastHit {
            tick: self.last_tick,
            damage_type: self.last_damage_type,
        })
    }

    /// The count of the hits that passed the tests of DMG-HIT-001, also those that the cooldown
    /// rejected. A reader compares it with an earlier count to see a new strike.
    #[must_use]
    pub fn strikes(&self) -> u16 {
        self.strikes
    }

    /// Counts one strike. The count wraps: a reader only compares it for a change, and a
    /// saturated count would hide every later strike.
    pub fn strike(&mut self) {
        self.strikes = self.strikes.wrapping_add(1);
    }

    /// Records an accepted hit in `tick`. A damage type id that does not fit the record is kept
    /// as the largest id that does; registries hold far fewer types.
    pub fn accept(&mut self, tick: u64, damage_type: u32) {
        self.last_tick = tick as u32;
        self.last_damage_type = u16::try_from(damage_type)
            .unwrap_or(NO_HIT - 1)
            .min(NO_HIT - 1);
    }
}
