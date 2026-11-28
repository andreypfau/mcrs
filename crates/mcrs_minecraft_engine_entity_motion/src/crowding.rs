use mcrs_minecraft_physics::PushRules;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Crowding {
    pub rules: PushRules,
    pub damage_type: Option<u32>,
}
