use mcrs_minecraft_damage::Vitals;
use mcrs_minecraft_physics::GridFlags;

pub(crate) fn living_flags(vitals: Option<&Vitals>) -> GridFlags {
    if vitals.is_none_or(Vitals::is_alive) {
        GridFlags::PUSHABLE
    } else {
        GridFlags::empty()
    }
}
