use crate::EntityKey;

/// The entities that caused one event: the entity that did it and the entity that carried it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Cause {
    pub direct: Option<EntityKey>,
    pub causing: Option<EntityKey>,
}

impl Cause {
    #[must_use]
    pub const fn none() -> Self {
        Self {
            direct: None,
            causing: None,
        }
    }

    #[must_use]
    pub const fn new(causing: EntityKey, direct: EntityKey) -> Self {
        Self {
            direct: Some(direct),
            causing: Some(causing),
        }
    }

    #[must_use]
    pub const fn with_causing(mut self, causing: EntityKey) -> Self {
        self.causing = Some(causing);
        self
    }

    #[must_use]
    pub const fn with_direct(mut self, direct: EntityKey) -> Self {
        self.direct = Some(direct);
        self
    }

    #[must_use]
    pub const fn is_none(&self) -> bool {
        self.direct.is_none() && self.causing.is_none()
    }
}
