use bevy::prelude::*;
use wgpu::{ComputePassTimestampWrites, QuerySet};

#[derive(Clone, Copy)]
pub enum PassSlot {
    Cull,
    Hiz,
    CullSecond,
    CullSections,
    CullBlended,
    CullQuads,
    CullQuadsSecond,
    EntityScatter,
    EntityCull,
    EntityPose,
    EntityExpand,
}

impl PassSlot {
    pub const ALL: [Self; 11] = [
        Self::Cull,
        Self::Hiz,
        Self::CullSecond,
        Self::CullSections,
        Self::CullBlended,
        Self::CullQuads,
        Self::CullQuadsSecond,
        Self::EntityScatter,
        Self::EntityCull,
        Self::EntityPose,
        Self::EntityExpand,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::Cull => "cull",
            Self::Hiz => "hiz",
            Self::CullSecond => "cull second",
            Self::CullSections => "cull sections",
            Self::CullBlended => "cull blended",
            Self::CullQuads => "cull quads",
            Self::CullQuadsSecond => "cull quads second",
            Self::EntityScatter => "entity scatter",
            Self::EntityCull => "entity cull",
            Self::EntityPose => "entity pose",
            Self::EntityExpand => "entity expand",
        }
    }
}

#[derive(Resource, Default)]
pub struct PassTimestamps {
    /// The query set and the index of this frame's first query, or `None` when no pass is timed.
    pub queries: Option<(QuerySet, u32)>,
}

impl PassTimestamps {
    pub fn compute(&self, slot: PassSlot) -> Option<ComputePassTimestampWrites<'_>> {
        let (query_set, first) = self.queries.as_ref()?;
        let begin = first + 2 * slot as u32;
        Some(ComputePassTimestampWrites {
            query_set,
            beginning_of_pass_write_index: Some(begin),
            end_of_pass_write_index: Some(begin + 1),
        })
    }
}
