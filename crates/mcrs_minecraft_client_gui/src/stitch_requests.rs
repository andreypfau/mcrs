use bevy::prelude::*;

use crate::stitch_request::StitchRequest;

#[derive(Resource, Default)]
pub struct StitchRequests(Vec<StitchRequest>);

impl StitchRequests {
    pub(crate) fn iter(&self) -> impl Iterator<Item = StitchRequest> + '_ {
        self.0.iter().copied()
    }

    fn insert(&mut self, request: StitchRequest) {
        let region = request.region();
        let Some(known) = self.0.iter_mut().find(|known| known.region() == region) else {
            self.0.push(request);
            return;
        };
        if let (
            StitchRequest::Background {
                extent: known_extent,
                ..
            },
            StitchRequest::Background { extent, .. },
        ) = (known, request)
        {
            *known_extent = known_extent.max(extent);
        }
    }
}

impl Extend<StitchRequest> for StitchRequests {
    fn extend<T: IntoIterator<Item = StitchRequest>>(&mut self, requests: T) {
        for request in requests {
            self.insert(request);
        }
    }
}
