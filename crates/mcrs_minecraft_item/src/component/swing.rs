use bevy_ecs::component::Component;

#[derive(Clone, Debug, Component)]
pub struct SwingAnimation {
    pub kind: SwingAnimationKind,
    pub duration: u32,
}

impl SwingAnimation {
    pub const DEFAULT: Self = Self {
        kind: SwingAnimationKind::Whack,
        duration: 6,
    };

    pub const fn new(kind: SwingAnimationKind, duration: u32) -> Self {
        Self { kind, duration }
    }
}

impl Default for SwingAnimation {
    fn default() -> Self {
        Self::DEFAULT
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub enum SwingAnimationKind {
    #[default]
    None,
    Whack,
    Stab,
}
