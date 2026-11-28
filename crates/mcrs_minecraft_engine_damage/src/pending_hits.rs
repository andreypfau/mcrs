use bevy_ecs::resource::Resource;
use bevy_ecs::system::Res;

use crate::DamageRequest;

#[derive(Resource, Clone, Debug, Default)]
pub struct PendingHits {
    pub(crate) requests: Vec<DamageRequest>,
}

impl PendingHits {
    pub fn push(&mut self, request: DamageRequest) {
        self.requests.push(request);
    }

    #[must_use]
    pub fn requests(&self) -> &[DamageRequest] {
        &self.requests
    }

    #[must_use]
    pub fn requests_mut(&mut self) -> &mut [DamageRequest] {
        &mut self.requests
    }

    pub fn retain(&mut self, keep: impl FnMut(&DamageRequest) -> bool) {
        self.requests.retain(keep);
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.requests.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.requests.is_empty()
    }
}

pub(crate) fn has_pending_hits(pending: Res<PendingHits>) -> bool {
    !pending.is_empty()
}
