use crate::Collider;
use crate::EntityCollider;

/// The buffers that one task reuses between steps, so a step does not allocate.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Scratch {
    pub(crate) colliders: Vec<Collider>,
    pub(crate) step_colliders: Vec<Collider>,
    pub(crate) entities: Vec<EntityCollider>,
    pub(crate) lines: Vec<f64>,
    pub(crate) candidates: Vec<f32>,
}

impl Scratch {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}
