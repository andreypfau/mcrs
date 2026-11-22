use mcrs_minecraft_damage::HitRecord;
use mcrs_minecraft_damage::Vitals;
use mcrs_minecraft_physics::EntityState;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Sensed<'a> {
    pub(crate) state: &'a EntityState,
    pub(crate) vitals: Option<&'a Vitals>,
    pub(crate) hits: Option<&'a HitRecord>,
}

impl Sensed<'_> {
    pub(crate) fn is_alive(&self) -> bool {
        self.vitals.is_none_or(Vitals::is_alive)
    }
}
