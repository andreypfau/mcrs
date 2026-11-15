use bevy_ecs::entity::Entity;
use bevy_ecs::event::EntityEvent;

use crate::Denial;
use crate::Request;

/// Triggered on the actor before the plan of each [`RequestKind::Action`](crate::RequestKind)
/// request. An observer reads the request and can deny it; a denied request changes nothing. An
/// observer must not change a slot that the request names (ITX-ATM-011).
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

    /// Denies the request. The first denial wins; a later one changes nothing (ITX-ATM-009).
    pub fn deny(&mut self, denial: Denial) {
        if self.denial.is_none() {
            self.denial = Some(denial);
        }
    }

    pub(crate) fn denial(&self) -> Option<Denial> {
        self.denial
    }
}
