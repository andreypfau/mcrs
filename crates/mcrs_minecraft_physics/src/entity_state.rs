use mcrs_minecraft_entity_key::EntityKey;

use crate::Body;
use crate::HitRecord;
use crate::Living;
use crate::MovePath;
use crate::Rest;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EntityState {
    pub key: EntityKey,
    pub body: Body,
    pub living: Option<Living>,
    pub hits: HitRecord,
    pub no_gravity: bool,
    pub age: u32,
    pub phase: u8,
    pub rest: Rest,
    pub last_path: MovePath,
}

impl EntityState {
    #[must_use]
    pub fn new(key: EntityKey, body: Body, living: Option<Living>) -> Self {
        Self {
            key,
            body,
            living,
            hits: HitRecord::default(),
            no_gravity: false,
            age: 0,
            phase: 0,
            rest: Rest::Awake,
            last_path: MovePath::at(body.position),
        }
    }
}
