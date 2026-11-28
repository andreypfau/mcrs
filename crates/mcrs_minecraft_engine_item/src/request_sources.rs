use bevy_ecs::resource::Resource;

use crate::RegisterError;
use crate::RequestSource;

#[derive(Resource, Default)]
pub(crate) struct RequestSources {
    next: u16,
}

impl RequestSources {
    pub(crate) fn allocate(&mut self) -> Result<RequestSource, RegisterError> {
        let source = RequestSource(self.next);
        self.next = self
            .next
            .checked_add(1)
            .ok_or(RegisterError::SourcesExhausted)?;
        Ok(source)
    }
}
