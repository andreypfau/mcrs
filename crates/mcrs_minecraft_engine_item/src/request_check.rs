use bevy_ecs::entity::Entity;
use bevy_ecs::event::EntityEvent;

use crate::Denial;
use crate::Request;

#[derive(EntityEvent, Clone, Debug, PartialEq)]
pub struct RequestCheck {
    #[event_target]
    pub actor: Entity,
    request: Request,
    denial: Option<Denial>,
}

impl RequestCheck {
    pub(crate) fn new(actor: Entity, request: Request) -> Self {
        Self {
            actor,
            request,
            denial: None,
        }
    }

    #[must_use]
    pub fn request(&self) -> &Request {
        &self.request
    }

    pub fn deny(&mut self, denial: Denial) {
        if self.denial.is_none() {
            self.denial = Some(denial);
        }
    }

    pub(crate) fn denial(&self) -> Option<Denial> {
        self.denial
    }
}
