use std::ops::DerefMut;

use bevy::math::{DMat4, DQuat, DVec3, Rect, Vec2, Vec3};
use bevy::prelude::*;
use bevy::transform::TransformSystems;
use mcrs_minecraft_client_gui::sign_text::{SignQuad, span_quads};
use mcrs_minecraft_client_gui::text::text_spans;
use mcrs_minecraft_entity::entity_type_keys::EntityType;
use mcrs_minecraft_entity_render::{EntityClock, EntitySlotSystems, EntityViews};
use mcrs_minecraft_entity_visual::{
    CameraEntity, ClientEntity, EntityLight, Motion, NameTag, SyncedData, box_height, name_text,
};
use mcrs_minecraft_render::name_tag::{NameTagVertex, NameTags, SOLID_UV};

use crate::glyph_page::{GlyphPage, TextAssets, ensure_loaded, linear};
use crate::player_skins::ProfileSkins;

pub const SCALE: f32 = 0.025;
pub const MAX_DISTANCE_SQUARED: f64 = 64.0 * 64.0;
pub const CROUCHING_DISTANCE_SQUARED: f64 = 1024.0;
pub const ROW_HEIGHT: f32 = 9.0;
const LIFT: f64 = 0.5;
pub const BACKGROUND_OPACITY: f32 = 0.25;
const SECOND_DRAW_MIN_LIGHT: u8 = 2;
const WHITE: u32 = 0xFFFF_FFFF;

#[derive(Clone, Copy, Debug, Default)]
pub struct TagInputs {
    pub distance_squared: f64,
    pub crouching: bool,
    pub player: bool,
    pub named: bool,
    pub name_visible: bool,
    pub targeted: bool,
    pub camera_entity: bool,
    pub hidden: bool,
    pub has_passenger: bool,
    pub hud_hidden: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TagDraw {
    Through,
    Crouching,
}

#[must_use]
pub fn tag_draw(inputs: &TagInputs) -> Option<TagDraw> {
    let shown = inputs.named
        && (inputs.player || inputs.name_visible || inputs.targeted)
        && inputs.distance_squared < MAX_DISTANCE_SQUARED
        && (!inputs.crouching || inputs.distance_squared < CROUCHING_DISTANCE_SQUARED)
        && !inputs.hud_hidden
        && !inputs.camera_entity
        && !inputs.hidden
        && !inputs.has_passenger;
    shown.then_some(if inputs.crouching {
        TagDraw::Crouching
    } else {
        TagDraw::Through
    })
}

#[must_use]
pub fn anchor(position: DVec3, box_height: f64) -> DVec3 {
    position + DVec3::new(0.0, box_height + LIFT, 0.0)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TagLayout {
    pub line_origin: Vec2,
    pub background: Rect,
}

#[must_use]
pub fn layout(width: f32) -> TagLayout {
    let x = -width / 2.0;
    TagLayout {
        line_origin: Vec2::new(x, 0.0),
        background: Rect::from_corners(
            Vec2::new(x - 1.0, -1.0),
            Vec2::new(x + width + 1.0, ROW_HEIGHT),
        ),
    }
}

#[must_use]
pub fn text_alpha(opacity: f32) -> f32 {
    ((opacity + 0.75) / 2.0).max(0.5)
}

#[must_use]
pub fn tag_matrix(camera: DQuat) -> DMat4 {
    let scale = f64::from(SCALE);
    DMat4::from_quat(camera) * DMat4::from_scale(DVec3::new(scale, -scale, scale))
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Face {
    Solid,
    Glyph(SignQuad),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Piece {
    pub corners: [Vec3; 4],
    pub face: Face,
    pub rgba: [f32; 4],
    pub light: [f32; 2],
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct TagPieces {
    pub through: Vec<Piece>,
    pub world: Vec<Piece>,
}

fn levels(light: EntityLight, floor: u8) -> [f32; 2] {
    [light.block.max(floor), light.sky.max(floor)].map(|level| f32::from(level.min(15)) * 16.0)
}

#[must_use]
pub fn pieces(
    draw: TagDraw,
    layout: &TagLayout,
    matrix: &DMat4,
    glyphs: &[SignQuad],
    light: EntityLight,
    opacity: f32,
) -> TagPieces {
    let corner = |x: f32, y: f32| {
        matrix
            .transform_point3(DVec3::new(f64::from(x), f64::from(y), 0.0))
            .as_vec3()
    };
    let rect = layout.background;
    let background = Piece {
        corners: [
            corner(rect.min.x, rect.min.y),
            corner(rect.max.x, rect.min.y),
            corner(rect.max.x, rect.max.y),
            corner(rect.min.x, rect.max.y),
        ],
        face: Face::Solid,
        rgba: [0.0, 0.0, 0.0, opacity],
        light: levels(light, 0),
    };
    let line = |alpha: f32, floor: u8| {
        glyphs.iter().map(move |quad| {
            let [red, green, blue] = linear(quad.color);
            Piece {
                corners: quad.corners,
                face: Face::Glyph(*quad),
                rgba: [red, green, blue, alpha],
                light: levels(light, floor),
            }
        })
    };
    let text = text_alpha(opacity);
    match draw {
        TagDraw::Through => TagPieces {
            through: [background].into_iter().chain(line(text, 0)).collect(),
            world: line(1.0, SECOND_DRAW_MIN_LIGHT).collect(),
        },
        TagDraw::Crouching => TagPieces {
            through: Vec::new(),
            world: [background].into_iter().chain(line(text, 0)).collect(),
        },
    }
}

pub struct NameTagPlugin;

impl Plugin for NameTagPlugin {
    fn build(&self, app: &mut App) {
        ensure_loaded(app);
        app.add_systems(
            PostUpdate,
            sync_name_tags
                .run_if(resource_exists::<TextAssets>)
                .after(EntitySlotSystems)
                .after(TransformSystems::Propagate),
        );
    }
}

#[derive(Default)]
struct Scratch {
    through: Vec<NameTagVertex>,
    world: Vec<NameTagVertex>,
    glyphs: Vec<SignQuad>,
}

fn holds(tags: &NameTags, through: &[NameTagVertex], world: &[NameTagVertex]) -> bool {
    tags.through as usize == through.len()
        && tags.vertices.len() == through.len() + world.len()
        && tags.vertices[..through.len()] == *through
        && tags.vertices[through.len()..] == *world
}

fn publish<T: DerefMut<Target = NameTags>>(
    tags: &mut T,
    through: &[NameTagVertex],
    world: &[NameTagVertex],
) {
    if holds(tags, through, world) {
        return;
    }
    tags.vertices.clear();
    tags.vertices.extend_from_slice(through);
    tags.vertices.extend_from_slice(world);
    tags.through = through.len() as u32;
    tags.revision += 1;
}

#[cfg_attr(
    mcrs_lint,
    expect(
        mcrs::alloc_in_tick,
        reason = "the spans and the pieces are built only for the few named entities that show a tag near the camera"
    )
)]
#[cfg_attr(mcrs_lint, mcrs::alloc_sites(4))]
fn sync_name_tags(
    mut assets: ResMut<TextAssets>,
    mut images: ResMut<Assets<Image>>,
    camera: Query<&GlobalTransform, With<Camera3d>>,
    clock: Option<Res<EntityClock>>,
    skins: Option<Res<ProfileSkins>>,
    entities: Query<(
        &ClientEntity,
        &Motion,
        &SyncedData,
        Option<&NameTag>,
        Option<&EntityLight>,
        Option<&EntityViews>,
        Has<CameraEntity>,
    )>,
    mut tags: ResMut<NameTags>,
    mut scratch: Local<Scratch>,
) {
    let Ok(camera) = camera.single() else {
        publish(&mut tags, &[], &[]);
        return;
    };
    let eye = camera.translation().as_dvec3();
    let partial_tick = clock.map_or(0.0, |clock| clock.partial_tick);
    let matrix = tag_matrix(camera.rotation().as_dquat());
    let assets = &mut *assets;
    let Scratch {
        through,
        world,
        glyphs,
    } = &mut *scratch;
    through.clear();
    world.clear();
    for (entity, motion, data, host, light, views, camera_entity) in &entities {
        let player = entity.kind == EntityType::Player;
        let profile = skins
            .as_deref()
            .filter(|_| player)
            .and_then(|skins| skins.name(entity.uuid.as_u128()));
        let text = name_text(host, data.custom_name.as_ref(), profile);
        let position = motion.frame_position(partial_tick);
        let inputs = TagInputs {
            distance_squared: eye.distance_squared(position),
            crouching: data.is_sneaking(),
            player,
            named: text.is_some(),
            name_visible: data.name_visible,
            targeted: false,
            camera_entity,
            hidden: data.is_invisible()
                || views.is_some_and(|views| views.0 & EntityViews::WORLD.0 == 0),
            has_passenger: false,
            hud_hidden: false,
        };
        let (Some(draw), Some(text)) = (tag_draw(&inputs), text) else {
            continue;
        };
        let spans = text_spans(text, &assets.language, WHITE);
        let tag = layout(assets.font.width(&spans) as f32);
        glyphs.clear();
        span_quads(
            &assets.font,
            &spans,
            tag.line_origin,
            None,
            0.0,
            (&matrix, 0),
            glyphs,
        );
        let light = light.copied().unwrap_or(EntityLight::OPEN_SKY);
        let placed = pieces(draw, &tag, &matrix, glyphs, light, BACKGROUND_OPACITY);
        let at = anchor(position, box_height(entity, data));
        let block = at.floor();
        let around = (at - block).as_vec3();
        let cell = block.as_ivec3().to_array();
        for (list, out) in [(placed.through, &mut *through), (placed.world, &mut *world)] {
            for piece in &list {
                push_piece(piece, cell, around, &mut assets.page, out);
            }
        }
    }
    assets.flush(&mut images);
    if tags.texture.as_ref() != Some(&assets.image) {
        tags.texture = Some(assets.image.clone());
    }
    publish(&mut tags, through, world);
}

fn push_piece(
    piece: &Piece,
    cell: [i32; 3],
    around: Vec3,
    page: &mut GlyphPage,
    out: &mut Vec<NameTagVertex>,
) {
    let uv = match piece.face {
        Face::Solid => [SOLID_UV; 4],
        Face::Glyph(quad) => match page.uv_corners(&quad) {
            Some(uv) => uv,
            None => return,
        },
    };
    let vertex = |index: usize| NameTagVertex {
        block: cell,
        offset: (around + piece.corners[index]).to_array(),
        uv: uv[index],
        color: piece.rgba,
        light: piece.light,
    };
    for index in [0, 1, 2, 0, 2, 3] {
        out.push(vertex(index));
    }
}

#[cfg(test)]
mod tests {
    use mcrs_minecraft_client_gui::font_glyph::ImageSource;

    use super::*;

    #[test]
    fn the_tag_shows_by_name_flag_distance_and_the_hiding_rules() {
        let base = TagInputs {
            distance_squared: 100.0,
            named: true,
            name_visible: true,
            ..TagInputs::default()
        };
        let through = Some(TagDraw::Through);
        let crouching = Some(TagDraw::Crouching);
        type Change = fn(&mut TagInputs);
        let cases: [(&str, Change, Option<TagDraw>); 15] = [
            ("a named mob with the flag", |_| {}, through),
            ("no name", |i| i.named = false, None),
            ("no flag and no target", |i| i.name_visible = false, None),
            (
                "the crosshair target without the flag",
                |i| {
                    i.name_visible = false;
                    i.targeted = true;
                },
                through,
            ),
            (
                "a player ignores the flag",
                |i| {
                    i.name_visible = false;
                    i.player = true;
                },
                through,
            ),
            (
                "a player with no name",
                |i| {
                    i.named = false;
                    i.player = true;
                },
                None,
            ),
            (
                "just inside 64 blocks",
                |i| i.distance_squared = 4095.9,
                through,
            ),
            ("at 64 blocks", |i| i.distance_squared = 4096.0, None),
            (
                "a non-crouching mob keeps the far range",
                |i| i.distance_squared = 2000.0,
                through,
            ),
            (
                "crouching just inside 32 blocks",
                |i| {
                    i.crouching = true;
                    i.distance_squared = 1023.9;
                },
                crouching,
            ),
            (
                "crouching at 32 blocks",
                |i| {
                    i.crouching = true;
                    i.distance_squared = 1024.0;
                },
                None,
            ),
            (
                "the camera entity",
                |i| {
                    i.camera_entity = true;
                    i.player = true;
                },
                None,
            ),
            ("hidden from the viewer", |i| i.hidden = true, None),
            ("a passenger on it", |i| i.has_passenger = true, None),
            ("the HUD hidden", |i| i.hud_hidden = true, None),
        ];
        for (label, change, expected) in cases {
            let mut inputs = base;
            change(&mut inputs);
            assert_eq!(tag_draw(&inputs), expected, "{label}");
        }
    }

    #[test]
    fn publishing_bumps_the_revision_only_when_the_vertices_differ_and_clears_with_no_tags() {
        let at = |x: f32| NameTagVertex {
            offset: [x, 0.0, 0.0],
            ..NameTagVertex::default()
        };
        let mut tags = Box::new(NameTags::default());
        publish(&mut tags, &[], &[]);
        assert_eq!(tags.revision, 0, "nothing to draw, nothing changed");

        publish(&mut tags, &[at(1.0)], &[at(2.0)]);
        assert_eq!((tags.revision, tags.through), (1, 1));
        publish(&mut tags, &[at(1.0)], &[at(2.0)]);
        assert_eq!(tags.revision, 1, "the same frame");
        publish(&mut tags, &[at(1.0), at(2.0)], &[]);
        assert_eq!(tags.revision, 2, "the same vertices in another split");
        publish(&mut tags, &[at(1.0), at(3.0)], &[]);
        assert_eq!(tags.revision, 3, "a tag moved");

        publish(&mut tags, &[], &[]);
        assert_eq!(
            (tags.revision, tags.vertices.len(), tags.through),
            (4, 0, 0),
            "no camera: the last frame is gone"
        );
    }

    #[test]
    fn the_background_wraps_the_centred_line_and_the_anchor_lifts_half_a_block() {
        let even = layout(59.0);
        assert_eq!(
            even.line_origin,
            Vec2::new(-29.5, 0.0),
            "w / 2 is not rounded"
        );
        assert_eq!(even.background.min, Vec2::new(-30.5, -1.0));
        assert_eq!(even.background.max, Vec2::new(30.5, 9.0));
        assert!(
            (even.background.width() * SCALE - 1.525).abs() < 1e-6,
            "the CEREN-NAME-006 example"
        );
        let empty = layout(0.0);
        assert_eq!(empty.background.size(), Vec2::new(2.0, 10.0));
        let lifted = anchor(DVec3::new(1.0, 2.0, 3.0), 1.8);
        assert!(lifted.distance(DVec3::new(1.0, 4.3, 3.0)) < 1e-9);
    }

    fn glyph(color: u32) -> SignQuad {
        SignQuad {
            corners: [Vec3::ZERO; 4],
            glyph: 'a',
            source: ImageSource::Missing,
            color,
            light: 0,
        }
    }

    #[test]
    fn a_tag_draws_the_background_and_the_line_by_its_draw_with_the_alphas_of_the_opacity() {
        let matrix = tag_matrix(DQuat::from_euler(EulerRot::YXZ, 0.5, -0.3, 0.0));
        let tag = layout(59.0);
        let glyphs = [glyph(0xFFFF_0000), glyph(0xFF00_00FF)];
        let light = EntityLight { block: 0, sky: 1 };
        let alphas =
            |pieces: &[Piece]| pieces.iter().map(|piece| piece.rgba[3]).collect::<Vec<_>>();
        let faces = |pieces: &[Piece]| {
            pieces
                .iter()
                .map(|piece| matches!(piece.face, Face::Solid))
                .collect::<Vec<_>>()
        };

        let through = pieces(TagDraw::Through, &tag, &matrix, &glyphs, light, 0.25);
        assert_eq!(faces(&through.through), [true, false, false]);
        assert_eq!(alphas(&through.through), [0.25, 0.5, 0.5]);
        assert_eq!(faces(&through.world), [false, false], "no background");
        assert_eq!(alphas(&through.world), [1.0, 1.0], "opaque");
        assert_eq!(
            through.world[0].rgba[..3],
            linear(0xFFFF_0000),
            "keeps its colour"
        );
        assert_eq!(through.through[0].rgba[..3], [0.0; 3], "black");
        assert_eq!(through.through[1].light, [0.0, 16.0]);
        assert_eq!(through.world[0].light, [32.0, 32.0], "at least level 2");

        let crouching = pieces(TagDraw::Crouching, &tag, &matrix, &glyphs, light, 0.8);
        assert!(crouching.through.is_empty(), "one draw");
        assert_eq!(faces(&crouching.world), [true, false, false]);
        let rounded: Vec<f32> = alphas(&crouching.world)
            .iter()
            .map(|alpha| (alpha * 1000.0).round() / 1000.0)
            .collect();
        assert_eq!(rounded, [0.8, 0.775, 0.775]);

        let low = pieces(TagDraw::Crouching, &tag, &matrix, &glyphs, light, 0.0);
        assert_eq!(
            alphas(&low.world),
            [0.0, 0.5, 0.5],
            "the text alpha is at least 0.5"
        );
    }

    #[test]
    fn the_background_faces_the_camera_and_measures_the_layout_in_blocks() {
        let rotation = DQuat::from_euler(EulerRot::YXZ, 0.9, -0.4, 0.0);
        let matrix = tag_matrix(rotation);
        let tag = layout(59.0);
        let drawn = pieces(
            TagDraw::Crouching,
            &tag,
            &matrix,
            &[],
            EntityLight::OPEN_SKY,
            0.25,
        );
        let [a, b, c, d] = drawn.world[0].corners.map(|corner| corner.as_dvec3());
        let (right, down) = (b - a, d - a);
        let back = (rotation * DVec3::Z).normalize();
        assert!(
            right.dot(back).abs() < 1e-6 && down.dot(back).abs() < 1e-6,
            "in the camera plane"
        );
        assert!(
            (right - (rotation * DVec3::X) * 1.525).length() < 1e-5,
            "61 units to the right"
        );
        assert!(
            (down + (rotation * DVec3::Y) * 0.25).length() < 1e-5,
            "10 units down"
        );
        assert!((c - b - down).length() < 1e-6, "a rectangle");
    }
}
