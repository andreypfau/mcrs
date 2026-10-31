use bevy_ecs::prelude::{Component, Entity, Message, MessageWriter, Query};
use bevy_ecs::system::SystemParam;
use mcrs_minecraft_core::SectionPos;

use crate::world::dimension::InDimension;

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub enum SectionStage {
    Loading,
    Generating,
    Loaded,
    Unloading,
}

/// Written for every stage a section enters, its spawn included. The stage stays
/// the truth: by the time a reader gets to the message the section may have moved
/// on again, so a reader that keeps state decides from the stage it finds.
#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
pub struct SectionStageChanged {
    pub section: Entity,
    pub pos: SectionPos,
    pub dim: Entity,
    pub from: Option<SectionStage>,
    pub to: SectionStage,
}

impl SectionStageChanged {
    pub fn spawned(section: Entity, pos: SectionPos, dim: Entity, stage: SectionStage) -> Self {
        Self {
            section,
            pos,
            dim,
            from: None,
            to: stage,
        }
    }

    pub fn landed(&self) -> bool {
        self.to == SectionStage::Loaded && self.from != Some(SectionStage::Loaded)
    }

    pub fn left(&self) -> bool {
        self.from == Some(SectionStage::Loaded) && self.to != SectionStage::Loaded
    }
}

#[derive(SystemParam)]
pub struct SectionStages<'w, 's> {
    sections: Query<
        'w,
        's,
        (
            &'static mut SectionStage,
            &'static SectionPos,
            &'static InDimension,
        ),
    >,
    changes: MessageWriter<'w, SectionStageChanged>,
}

impl SectionStages<'_, '_> {
    pub fn get(&self, section: Entity) -> Option<SectionStage> {
        self.sections.get(section).ok().map(|(stage, _, _)| *stage)
    }

    /// For a section spawned through `Commands`, whose stage no query can see yet.
    pub fn spawned(&mut self, section: Entity, pos: SectionPos, dim: Entity, stage: SectionStage) {
        self.changes
            .write(SectionStageChanged::spawned(section, pos, dim, stage));
    }

    pub fn set(&mut self, section: Entity, to: SectionStage) {
        let Ok((mut stage, pos, dim)) = self.sections.get_mut(section) else {
            return;
        };
        let from = *stage;
        if from == to {
            return;
        }
        *stage = to;
        self.changes.write(SectionStageChanged {
            section,
            pos: *pos,
            dim: dim.0,
            from: Some(from),
            to,
        });
    }
}
