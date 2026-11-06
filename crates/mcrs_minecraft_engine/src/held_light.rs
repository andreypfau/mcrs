use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::Query;
use mcrs_minecraft_chunk::{BlockLight, LightLayer, LightStorage, SkyLight};
use mcrs_minecraft_engine_world::world::light_gate::SectionRelit;
use mcrs_voxel::SectionPos;

use crate::light_codec::SectionLight;
use crate::stored_column::StoredColumn;

/// The light of a column that the clients hold, one row for each section from the lowest up.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct HeldLight {
    min_section_y: i32,
    rows: Vec<HeldRow>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct HeldRow {
    block: Option<LightStorage>,
    sky: Option<LightStorage>,
}

impl HeldLight {
    pub(crate) fn of(stored: &StoredColumn) -> Self {
        Self {
            min_section_y: stored.min_section_y,
            rows: stored
                .sections
                .iter()
                .map(|section| HeldRow {
                    block: section.block_light.as_ref().map(|light| light.0.clone()),
                    sky: section.sky_light.as_ref().map(|light| light.0.clone()),
                })
                .collect(),
        }
    }

    /// Takes the light of `section` in `layer` as held, and tells whether the clients held
    /// other light there.
    fn take(&mut self, section: SectionPos, layer: LightLayer, light: &LightStorage) -> bool {
        let row = section
            .y
            .checked_sub(self.min_section_y)
            .and_then(|index| usize::try_from(index).ok())
            .and_then(|index| self.rows.get_mut(index));
        let Some(row) = row else {
            return true;
        };
        let held = match layer {
            LightLayer::Block => &mut row.block,
            LightLayer::Sky => &mut row.sky,
        };
        if held.as_ref() == Some(light) {
            return false;
        }
        *held = Some(light.clone());
        true
    }
}

/// The sections of a held column whose light the dimension wrote again, by layer.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct RelitSections {
    pub(crate) block: Vec<Entity>,
    pub(crate) sky: Vec<Entity>,
}

impl RelitSections {
    pub(crate) fn note(&mut self, relit: &SectionRelit) {
        if relit.block && !self.block.contains(&relit.section) {
            self.block.push(relit.section);
        }
        if relit.sky && !self.sky.contains(&relit.section) {
            self.sky.push(relit.section);
        }
    }

    /// The light of the sections that the clients do not hold, which it takes as held.
    /// Without a record of what the clients hold, every relit section is a change to them.
    pub(crate) fn take_unheld<'a>(
        &self,
        mut held: Option<&mut HeldLight>,
        light: &'a Query<(&SectionPos, &BlockLight, &SkyLight)>,
    ) -> UnheldLight<'a> {
        let mut unheld = |sections: &[Entity], layer: LightLayer| -> Vec<SectionLight<'a>> {
            sections
                .iter()
                .filter_map(|section| {
                    let (pos, block, sky) = light.get(*section).ok()?;
                    let now = match layer {
                        LightLayer::Block => &block.0,
                        LightLayer::Sky => &sky.0,
                    };
                    let changed = held
                        .as_deref_mut()
                        .is_none_or(|held| held.take(*pos, layer, now));
                    changed.then_some(SectionLight {
                        y: pos.y,
                        light: now,
                    })
                })
                .collect()
        };
        UnheldLight {
            block: unheld(&self.block, LightLayer::Block),
            sky: unheld(&self.sky, LightLayer::Sky),
        }
    }
}

/// The light of relit sections that the clients do not hold, by layer.
pub(crate) struct UnheldLight<'a> {
    pub(crate) block: Vec<SectionLight<'a>>,
    pub(crate) sky: Vec<SectionLight<'a>>,
}

impl UnheldLight<'_> {
    pub(crate) fn is_empty(&self) -> bool {
        self.block.is_empty() && self.sky.is_empty()
    }
}
