use mcrs_minecraft_entity::mob_effect_keys::MobEffect;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PotionEffect {
    pub effect: MobEffect,
    pub amplifier: u8,
    pub duration: i32,
    pub ambient: bool,
    pub show_particles: bool,
    pub show_icon: bool,
}
