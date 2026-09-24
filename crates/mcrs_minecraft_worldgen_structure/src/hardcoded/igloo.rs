use bevy_math::IVec3;
use mcrs_minecraft_core::{Mirror, ResourceLocation};
use mcrs_minecraft_random::Random;
use mcrs_minecraft_random::legacy::LegacyRandom;
use mcrs_minecraft_worldgen_feature::placement::HeightmapName;
use mcrs_minecraft_worldgen_feature::template::{bounding_box, transform};

use super::on_top_of_chunk_centre;
use crate::frozen::{FrozenStructures, TemplateId};
use crate::orient::random_rotation;
use crate::piece::{IglooPiece, Piece};
use crate::site::{Context, Site, Stub};

pub const TEMPLATES: &[&str] = &["igloo/top", "igloo/middle", "igloo/bottom"];

const LAYOUT_HEIGHT: i32 = 90;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IglooTemplate {
    Top,
    Middle,
    Bottom,
}

impl IglooTemplate {
    pub fn location(self) -> ResourceLocation {
        ResourceLocation::minecraft(TEMPLATES[self as usize])
    }

    pub fn from_location(location: &ResourceLocation) -> Option<Self> {
        [Self::Top, Self::Middle, Self::Bottom]
            .into_iter()
            .find(|template| template.location() == *location)
    }

    pub fn id(self, frozen: &FrozenStructures) -> TemplateId {
        frozen.template_ids[&self.location()]
    }

    pub const fn pivot(self) -> IVec3 {
        match self {
            IglooTemplate::Top => IVec3::new(3, 5, 5),
            IglooTemplate::Middle => IVec3::new(1, 3, 1),
            IglooTemplate::Bottom => IVec3::new(3, 6, 7),
        }
    }

    pub const fn offset(self) -> IVec3 {
        match self {
            IglooTemplate::Top => IVec3::ZERO,
            IglooTemplate::Middle => IVec3::new(2, -3, 4),
            IglooTemplate::Bottom => IVec3::new(0, -3, -2),
        }
    }
}

pub fn site(ctx: &mut Context<'_>, _rng: &mut LegacyRandom) -> Option<(IVec3, Stub)> {
    on_top_of_chunk_centre(ctx, HeightmapName::WorldSurfaceWg)
}

/// The reference lowers every piece at placement by the live
/// `WORLD_SURFACE_WG` height under the entrance, which is the same column
/// for the three templates; the density column answers it here.
pub fn layout(ctx: &mut Context<'_>, mut site: Site) -> Vec<Piece> {
    let start = IVec3::new(
        ctx.chunk.min_block_x(),
        LAYOUT_HEIGHT,
        ctx.chunk.min_block_z(),
    );
    let rotation = random_rotation(&mut site.rng);
    let entrance = start
        + transform(
            IVec3::new(3, 0, 0),
            Mirror::None,
            rotation,
            IglooTemplate::Top.pivot(),
        );
    let lowered = ctx
        .world
        .free_height(entrance.x, entrance.z, HeightmapName::WorldSurfaceWg)
        - LAYOUT_HEIGHT
        - 1;
    let piece = |template: IglooTemplate, depth: i32| {
        let position = start + template.offset() - IVec3::new(0, depth, 0);
        let size = ctx.frozen.manifests[template.id(ctx.frozen).0 as usize].size;
        Piece::Igloo(IglooPiece {
            template,
            position,
            rotation,
            bounds: bounding_box(size, position, rotation, Mirror::None, template.pivot()),
            height: position.y + lowered,
        })
    };
    let mut pieces = Vec::new();
    if site.rng.next_f64() < 0.5 {
        let depth = site.rng.next_i32_bound(8) + 4;
        pieces.push(piece(IglooTemplate::Bottom, depth * 3));
        for i in 0..depth - 1 {
            pieces.push(piece(IglooTemplate::Middle, i * 3));
        }
    }
    pieces.push(piece(IglooTemplate::Top, 0));
    pieces
}
