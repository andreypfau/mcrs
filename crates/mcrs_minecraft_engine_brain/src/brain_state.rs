use bevy_ecs::component::Component;
use mcrs_minecraft_brain::BrainRules;
use mcrs_minecraft_brain::Panic;
use mcrs_minecraft_damage::LastHit;
use mcrs_minecraft_entity::EntityTypeData;
use mcrs_minecraft_mob_control::ControlInput;
use mcrs_minecraft_mob_control::LookDefaults;
use mcrs_minecraft_mob_control::MobControl;
use mcrs_minecraft_mob_control::Rotations;

#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct BrainState {
    pub kind: &'static EntityTypeData,
    pub rules: &'static BrainRules,
    pub control: MobControl,
    pub travelled: Option<ControlInput>,
    pub turned: Option<Rotations>,
    pub idle_time: u32,
    pub last_hit: Option<LastHit>,
    pub strikes: u16,
    pub hit_tick: Option<u64>,
    pub panic: Panic,
    pub age: Option<u32>,
    pub stalled: bool,
}

impl BrainState {
    #[must_use]
    pub fn new(kind: &'static EntityTypeData, rules: &'static BrainRules) -> Option<Self> {
        let mob = kind.mob()?;
        Some(Self {
            kind,
            rules,
            control: MobControl::new(LookDefaults {
                turn_speed: mob.head_turn_speed,
                pitch_limit: mob.pitch_limit,
            }),
            travelled: None,
            turned: None,
            idle_time: 0,
            last_hit: None,
            strikes: 0,
            hit_tick: None,
            panic: Panic::Calm,
            age: None,
            stalled: false,
        })
    }
}
