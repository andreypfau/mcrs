use bevy::color::Mix;
use bevy::prelude::*;
use bevy::text::FontSize;
use bevy::transform::TransformSystems;
use bevy::ui::{ComputedNode, UiSystems};
use mcrs_minecraft_network::columns::{BlockSource, ColumnStore};

use crate::player::PlayerCamera;

/// `LightDebugRenderer.MAX_RENDER_DIST`.
const RADIUS: i32 = 10;
/// The nearest labels the pool can hold. A 21-cube of lit cells reaches several
/// thousand, and every one of them is a laid-out text node.
const LABELS: usize = 512;
const FONT_SIZE: f32 = 22.0;
/// Distance at which a label draws at its font size, standing in for the
/// perspective a billboarded quad would have had.
const FULL_SIZE_AT: f32 = 8.0;
const SCALE_MIN: f32 = 0.7;
const SCALE_MAX: f32 = 1.6;

const BLOCK_DIM: Srgba = Srgba::rgb(0.671, 0.0, 0.0);
const BLOCK_BRIGHT: Srgba = Srgba::rgb(1.0, 1.0, 0.0);
const SKY_DIM: Srgba = Srgba::rgb(0.0, 0.0, 1.0);
const SKY_BRIGHT: Srgba = Srgba::rgb(0.0, 1.0, 1.0);

const DIGITS: [&str; 16] = [
    "0", "1", "2", "3", "4", "5", "6", "7", "8", "9", "10", "11", "12", "13", "14", "15",
];

#[derive(Resource, Clone, Copy, Default, Debug, PartialEq, Eq)]
pub enum LightLevels {
    #[default]
    Off,
    Block,
    Sky,
    Both,
}

impl LightLevels {
    pub fn visible(self) -> bool {
        self != Self::Off
    }

    fn next(self) -> Self {
        match self {
            Self::Off => Self::Block,
            Self::Block => Self::Sky,
            Self::Sky => Self::Both,
            Self::Both => Self::Off,
        }
    }

    fn block(self) -> bool {
        matches!(self, Self::Block | Self::Both)
    }

    fn sky(self) -> bool {
        matches!(self, Self::Sky | Self::Both)
    }
}

#[derive(Component)]
struct LightLabel;

struct Label {
    screen: Vec2,
    distance: f32,
    scale: f32,
    text: &'static str,
    color: Color,
}

pub struct LightLevelsPlugin;

impl Plugin for LightLevelsPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(if crate::config::light_levels() {
            LightLevels::Both
        } else {
            LightLevels::Off
        })
        .add_systems(Update, (toggle, sync_pool).chain())
        .add_systems(
            PostUpdate,
            render
                .after(TransformSystems::Propagate)
                .before(UiSystems::Layout)
                .run_if(|levels: Res<LightLevels>| levels.visible()),
        );
    }
}

fn toggle(keys: Res<ButtonInput<KeyCode>>, mut levels: ResMut<LightLevels>) {
    if keys.just_pressed(KeyCode::F4) {
        *levels = levels.next();
    }
}

fn sync_pool(
    levels: Res<LightLevels>,
    mut commands: Commands,
    labels: Query<Entity, With<LightLabel>>,
) {
    if !levels.is_changed() {
        return;
    }
    if !levels.visible() {
        for label in &labels {
            commands.entity(label).despawn();
        }
        return;
    }
    if !labels.is_empty() {
        return;
    }
    for _ in 0..LABELS {
        commands.spawn((
            LightLabel,
            Text::new(""),
            TextFont {
                font_size: FontSize::Px(FONT_SIZE),
                ..default()
            },
            TextColor(Color::WHITE),
            Node {
                position_type: PositionType::Absolute,
                ..default()
            },
            UiTransform::IDENTITY,
            Visibility::Hidden,
        ));
    }
}

fn label(
    camera: &Camera,
    eye: &GlobalTransform,
    at: Vec3,
    level: u8,
    dim: Srgba,
    bright: Srgba,
) -> Option<Label> {
    let screen = camera.world_to_viewport(eye, at).ok()?;
    let distance = eye.translation().distance(at);
    Some(Label {
        screen,
        distance,
        scale: scale_at(distance),
        text: DIGITS[level as usize],
        color: colour(level, dim, bright),
    })
}

/// `ARGB.srgbLerp`: the endpoints are mixed as they are written, not in linear
/// light.
fn colour(level: u8, dim: Srgba, bright: Srgba) -> Color {
    dim.mix(&bright, f32::from(level) / 15.0).into()
}

fn scale_at(distance: f32) -> f32 {
    (FULL_SIZE_AT / distance).clamp(SCALE_MIN, SCALE_MAX)
}

fn render(
    levels: Res<LightLevels>,
    store: Option<Res<ColumnStore>>,
    camera: Single<(&Camera, &GlobalTransform), With<PlayerCamera>>,
    mut labels: Query<
        (
            &mut UiTransform,
            &mut Visibility,
            &mut Text,
            &mut TextColor,
            &ComputedNode,
        ),
        With<LightLabel>,
    >,
) {
    let Some(store) = store else {
        return;
    };
    let (camera, eye) = *camera;
    let origin = eye.translation().floor().as_ivec3();
    let mut wanted = Vec::new();
    for dy in -RADIUS..=RADIUS {
        for dz in -RADIUS..=RADIUS {
            for dx in -RADIUS..=RADIUS {
                let block = origin + IVec3::new(dx, dy, dz);
                let light = store.light(block.x, block.y, block.z);
                let centre = block.as_vec3() + Vec3::splat(0.5);
                let block_level = light >> 4;
                if levels.block() && block_level != 0 {
                    wanted.extend(label(
                        camera,
                        eye,
                        centre,
                        block_level,
                        BLOCK_DIM,
                        BLOCK_BRIGHT,
                    ));
                }
                let sky_level = light & 0xf;
                if levels.sky() && sky_level != 15 {
                    wanted.extend(label(
                        camera,
                        eye,
                        centre - Vec3::Y * 0.25,
                        sky_level,
                        SKY_DIM,
                        SKY_BRIGHT,
                    ));
                }
            }
        }
    }

    // Nearest first to survive the cap, then reversed: the pool is drawn in
    // order, so the last one written lands on top of the ones behind it.
    wanted.sort_unstable_by(|a, b| a.distance.total_cmp(&b.distance));
    wanted.truncate(LABELS);
    wanted.reverse();

    let mut wanted = wanted.into_iter();
    for (mut transform, mut visibility, mut text, mut color, node) in &mut labels {
        let Some(label) = wanted.next() else {
            visibility.set_if_neq(Visibility::Hidden);
            continue;
        };
        visibility.set_if_neq(Visibility::Inherited);
        if text.0 != label.text {
            text.0 = label.text.to_owned();
        }
        color.set_if_neq(TextColor(label.color));
        // A node's transform sits at its centre, so half its own size is what
        // puts the digit on the cell rather than beside it.
        let half = node.size() * node.inverse_scale_factor() / 2.0;
        transform.set_if_neq(UiTransform {
            translation: Val2::px(label.screen.x - half.x, label.screen.y - half.y),
            scale: Vec2::splat(label.scale),
            ..UiTransform::IDENTITY
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_key_cycles_both_layers_on_and_back_off() {
        let seen: Vec<_> =
            std::iter::successors(Some(LightLevels::Off), |levels| Some(levels.next()))
                .take(5)
                .collect();
        assert_eq!(
            seen,
            [
                LightLevels::Off,
                LightLevels::Block,
                LightLevels::Sky,
                LightLevels::Both,
                LightLevels::Off,
            ]
        );
        assert!(!LightLevels::Off.visible());
        assert!(LightLevels::Both.block() && LightLevels::Both.sky());
        assert!(LightLevels::Block.block() && !LightLevels::Block.sky());
    }

    #[test]
    fn a_label_takes_the_colour_of_the_level_it_prints() {
        assert_eq!(Srgba::from(colour(0, SKY_DIM, SKY_BRIGHT)), SKY_DIM);
        assert_eq!(
            Srgba::from(colour(15, BLOCK_DIM, BLOCK_BRIGHT)),
            BLOCK_BRIGHT
        );
        assert!(scale_at(0.5) <= SCALE_MAX && scale_at(100.0) >= SCALE_MIN);
    }
}
