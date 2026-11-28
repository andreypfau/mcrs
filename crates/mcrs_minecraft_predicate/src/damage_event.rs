use crate::damage_source::DamageSource;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DamageEvent<E> {
    pub player: E,
    pub source: DamageSource<E>,
    pub dealt: f64,
    pub taken: f64,
    pub blocked: bool,
}
