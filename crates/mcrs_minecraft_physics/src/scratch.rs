use crate::Collider;
use crate::EntityCollider;
use crate::GridRow;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Scratch {
    pub(crate) colliders: Vec<Collider>,
    pub(crate) step_colliders: Vec<Collider>,
    pub(crate) entities: Vec<EntityCollider>,
    pub(crate) lines: Vec<f64>,
    pub(crate) candidates: Vec<f32>,
    pub(crate) neighbours: Vec<GridRow>,
    pub(crate) ranked: Vec<GridRow>,
}

impl Scratch {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}
