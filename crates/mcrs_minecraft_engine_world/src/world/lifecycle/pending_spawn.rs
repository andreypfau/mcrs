use mcrs_voxel::SectionPos;

#[derive(Clone, Copy)]
pub struct PendingSpawn {
    distance_sq: i32,
    pos: SectionPos,
}

impl PendingSpawn {
    pub(crate) fn nearest(pos: SectionPos, centers: &[SectionPos]) -> Self {
        let distance_sq = centers
            .iter()
            .map(|center| pos.distance_squared(**center))
            .min()
            .unwrap_or(0);
        Self { distance_sq, pos }
    }

    pub(crate) fn distance_sq(&self) -> i32 {
        self.distance_sq
    }

    #[must_use]
    pub fn pos(&self) -> SectionPos {
        self.pos
    }
}
