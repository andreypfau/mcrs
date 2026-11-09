use mcrs_minecraft_brain::BrainRules;
use mcrs_minecraft_brain::Panic;
use mcrs_minecraft_entity::EntityTypeData;
use mcrs_minecraft_mob_control::ControlInput;
use mcrs_minecraft_mob_control::LookDefaults;
use mcrs_minecraft_mob_control::MobControl;
use mcrs_minecraft_mob_control::Rotations;
use mcrs_minecraft_physics::LastHit;

/// The hot state of a mob with AI, in the mob column of its row: what the controls before and
/// after the travel and the view of the snapshot read and write between two ticks. The row
/// itself holds the position, the rotations and the hits; the type data holds the look limits.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MobColumn {
    pub kind: &'static EntityTypeData,
    /// The rules of the brain of the mob.
    pub rules: &'static BrainRules,
    pub control: MobControl,
    /// The input of the last step before the travel: the body rotation of that step runs at the
    /// start of the next tick, since the pass has no step after the travel.
    pub travelled: Option<ControlInput>,
    /// The rotations after that body rotation, whose head yaw and body yaw the next intent
    /// writes.
    pub turned: Option<Rotations>,
    pub idle_time: u32,
    /// The last accepted hit that the view saw.
    pub last_hit: Option<LastHit>,
    /// The strike count of the row that the view saw (GOAL-STR-004).
    pub strikes: u16,
    /// The AI tick before the first view that saw the last accepted hit. The hit record of the
    /// row counts ticks on the clock of the world, not on the clock of the AI.
    pub hit_tick: Option<u64>,
    pub panic: Panic,
    /// The age of the row that the view saw. The pass adds 1 in each tick in which the row
    /// ticks, so an equal age at the next view means a tick with no step.
    pub age: Option<u32>,
    /// The row did not tick in the tick before the last view.
    pub stalled: bool,
}

impl MobColumn {
    /// The column of a new mob of `kind` with a brain of `rules`; `None` when `kind` is not a mob
    /// type.
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
