use std::time::Duration;

use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use bevy::text::{FontSize, LineBreak};
use mcrs_voxel_math::{BlockPos, ColumnPos};
use mcrs_voxel_world::world::lifecycle::trace::{self, ColumnSample, ColumnStage};

use crate::player::Player;
use mcrs_voxel_world::entity::physics::Transform as PhysicsTransform;

/// Columns across the map. A player's view distance is 12 columns plus a
/// border, so this leaves room for the ring that is still catching up behind
/// the one being drawn.
const SPAN: i32 = 33;
const CELL: f32 = 8.0;
const GAP: f32 = 1.0;
const FONT_SIZE: f32 = 13.0;

/// The header's width changes with every count it prints, and the panel is
/// pinned to the right edge, so a width that follows the text drags the grid
/// sideways from frame to frame. Fixed here, wrapped in the header.
const PANEL_WIDTH: f32 = 600.0;

const BACKGROUND: Color = Color::srgba(0.06, 0.06, 0.08, 0.82);
const TEXT: Color = Color::srgb(0.878, 0.878, 0.878);
const UNTRACED: Color = Color::srgba(0.16, 0.16, 0.18, 0.9);

fn stage_color(stage: ColumnStage) -> Color {
    match stage {
        ColumnStage::Ticketed => Color::srgb(0.75, 0.15, 0.15),
        ColumnStage::Spawned => Color::srgb(0.85, 0.35, 0.10),
        ColumnStage::Queued => Color::srgb(0.90, 0.55, 0.10),
        ColumnStage::Generating => Color::srgb(0.92, 0.80, 0.15),
        ColumnStage::Loaded => Color::srgb(0.60, 0.80, 0.20),
        ColumnStage::Ready => Color::srgb(0.20, 0.75, 0.35),
        ColumnStage::Sent => Color::srgb(0.15, 0.70, 0.75),
        ColumnStage::Received => Color::srgb(0.25, 0.45, 0.90),
        ColumnStage::Meshed => Color::srgb(0.85, 0.87, 0.92),
    }
}

#[derive(Resource, Default)]
pub struct ChunkMap {
    visible: bool,
}

impl ChunkMap {
    pub fn visible(&self) -> bool {
        self.visible
    }
}

#[derive(Component)]
struct ChunkMapRoot;

#[derive(Component)]
struct ChunkMapHeader;

#[derive(Component)]
struct ChunkMapCell(usize);

pub struct ChunkMapPlugin;

impl Plugin for ChunkMapPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ChunkMap {
            visible: crate::config::chunk_map(),
        })
        .add_systems(Startup, spawn)
        .add_systems(Update, (toggle, render).chain());
    }
}

fn toggle(keys: Res<ButtonInput<KeyCode>>, mut map: ResMut<ChunkMap>) {
    if keys.just_pressed(KeyCode::F6) {
        map.visible = !map.visible;
    }
}

fn spawn(mut commands: Commands) {
    let font = TextFont {
        font_size: FontSize::Px(FONT_SIZE),
        ..default()
    };
    commands
        .spawn((
            ChunkMapRoot,
            Node {
                position_type: PositionType::Absolute,
                bottom: Val::Px(4.0),
                right: Val::Px(4.0),
                flex_direction: FlexDirection::Column,
                width: Val::Px(PANEL_WIDTH),
                row_gap: Val::Px(4.0),
                padding: UiRect::all(Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(BACKGROUND),
            Visibility::Hidden,
        ))
        .with_children(|root| {
            root.spawn((
                ChunkMapHeader,
                Text::new(""),
                font.clone(),
                TextLayout::new(Justify::Left, LineBreak::WordBoundary),
                TextColor(TEXT),
            ));

            root.spawn(Node {
                display: Display::Grid,
                grid_template_columns: RepeatedGridTrack::px(SPAN as u16, CELL),
                row_gap: Val::Px(GAP),
                column_gap: Val::Px(GAP),
                ..default()
            })
            .with_children(|grid| {
                let centre = (SPAN * SPAN / 2) as usize;
                for index in 0..(SPAN * SPAN) as usize {
                    grid.spawn((
                        ChunkMapCell(index),
                        Node {
                            width: Val::Px(CELL),
                            height: Val::Px(CELL),
                            border: UiRect::all(Val::Px(if index == centre { 1.0 } else { 0.0 })),
                            ..default()
                        },
                        BorderColor::all(Color::srgb(1.0, 0.2, 0.8)),
                        BackgroundColor(UNTRACED),
                    ));
                }
            });

            root.spawn(Node {
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(6.0),
                flex_wrap: FlexWrap::Wrap,
                ..default()
            })
            .with_children(|legend| {
                for stage in ColumnStage::ALL {
                    legend
                        .spawn(Node {
                            flex_direction: FlexDirection::Row,
                            align_items: AlignItems::Center,
                            column_gap: Val::Px(3.0),
                            ..default()
                        })
                        .with_children(|item| {
                            item.spawn((
                                Node {
                                    width: Val::Px(CELL),
                                    height: Val::Px(CELL),
                                    ..default()
                                },
                                BackgroundColor(stage_color(stage)),
                            ));
                            item.spawn((Text::new(stage.label()), font.clone(), TextColor(TEXT)));
                        });
                }
            });
        });
}

fn render(
    map: Res<ChunkMap>,
    mut root: Query<&mut Visibility, With<ChunkMapRoot>>,
    mut cells: Query<(&ChunkMapCell, &mut BackgroundColor)>,
    mut header: Query<&mut Text, With<ChunkMapHeader>>,
    player: Query<&PhysicsTransform, With<Player>>,
    mut samples: Local<Vec<ColumnSample>>,
    mut stages: Local<HashMap<ColumnPos, ColumnStage>>,
    mut sent: Local<Vec<Duration>>,
) {
    let Ok(mut visibility) = root.single_mut() else {
        return;
    };
    if !map.visible {
        visibility.set_if_neq(Visibility::Hidden);
        return;
    }
    visibility.set_if_neq(Visibility::Inherited);

    let Ok(player) = player.single() else {
        return;
    };
    let feet = BlockPos::from(player.translation);
    let centre = ColumnPos::new(feet.x >> 4, feet.z >> 4);
    let half = SPAN / 2;

    trace::snapshot(&mut samples);
    stages.clear();
    sent.clear();
    let mut counts = [0usize; ColumnStage::ALL.len()];
    let mut sources = [0usize; 2];
    let mut off_map = 0usize;
    let mut slowest: Option<(ColumnPos, ColumnStage, Duration)> = None;

    for sample in samples.iter() {
        counts[sample.stage as usize] += 1;
        if let Some(after) = sample.sent_after {
            sent.push(after);
        }
        match sample.source {
            Some("save") => sources[0] += 1,
            Some(_) => sources[1] += 1,
            None => {}
        }
        if sample.stage < ColumnStage::Meshed
            && slowest.is_none_or(|(_, _, worst)| sample.in_stage > worst)
        {
            slowest = Some((sample.pos, sample.stage, sample.in_stage));
        }
        let dx = sample.pos.x - centre.x;
        let dz = sample.pos.z - centre.z;
        if dx.abs() > half || dz.abs() > half {
            off_map += 1;
            continue;
        }
        stages.insert(sample.pos, sample.stage);
    }

    for (cell, mut color) in &mut cells {
        let row = cell.0 as i32 / SPAN;
        let column = cell.0 as i32 % SPAN;
        let pos = ColumnPos::new(centre.x - half + column, centre.z - half + row);
        let want = stages.get(&pos).copied().map_or(UNTRACED, stage_color);
        if color.0 != want {
            color.0 = want;
        }
    }

    let Ok(mut header) = header.single_mut() else {
        return;
    };
    sent.sort_unstable();
    let quantile = |at: f32| -> String {
        if sent.is_empty() {
            return "-".to_owned();
        }
        let index = ((sent.len() - 1) as f32 * at).round() as usize;
        duration(sent[index])
    };
    let tally = ColumnStage::ALL
        .iter()
        .map(|stage| format!("{} {}", stage.label(), counts[*stage as usize]))
        .collect::<Vec<_>>()
        .join("  ");
    let block = format!(
        "chunk map (F6)  {SPAN}x{SPAN} at ({}, {})  {} traced, {off_map} off-map\n\
         {tally}\n\
         ticket to sent  p50 {}  p95 {}  max {}   from disk {}, generated {}\n\
         {}",
        centre.x,
        centre.z,
        samples.len(),
        quantile(0.5),
        quantile(0.95),
        quantile(1.0),
        sources[0],
        sources[1],
        match slowest {
            Some((pos, stage, held)) => format!(
                "waiting longest  ({}, {}) in {} for {}",
                pos.x,
                pos.z,
                stage.label(),
                duration(held)
            ),
            None => "waiting longest  nothing pending".to_owned(),
        },
    );
    if header.0 != block {
        header.0 = block;
    }
}

fn duration(of: Duration) -> String {
    let ms = of.as_secs_f32() * 1000.0;
    if ms < 1000.0 {
        format!("{ms:.0}ms")
    } else {
        format!("{:.2}s", ms / 1000.0)
    }
}
