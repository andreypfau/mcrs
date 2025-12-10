use bevy::DefaultPlugins;
use bevy::a11y::AccessibilitySystems::Update;
use bevy::color::palettes::basic::{GREEN, PURPLE, RED};
use bevy::log::tracing_subscriber::filter::combinator::And;
use bevy::math::{Vec2, Vec3Swizzles};
use bevy::mesh::Mesh2d;
use bevy::prelude::{
    Assets, Camera2d, Circle, Color, ColorMaterial, Component, Mesh, MeshMaterial2d, Rectangle,
    Transform, Vec3,
};
use bevy_app::{App, FixedUpdate, Plugin, Startup};
use bevy_ecs::change_detection::{Mut, ResMut};
use bevy_ecs::prelude::{Commands, Entity, Query, Without};
use bevy_ecs::query::{Added, With};
use bevy_ecs::system::Res;
use mcrs_protocol::{ChunkColumnPos, ChunkPos, Position};
use mcrs_server::world::chunk::{ChunkIndex, ChunkStatus};
use mcrs_server::world::chunk_observer::{ChunkObserverPlugin, PlayerChunkObserver};
use std::collections::HashMap;
use bevy_inspector_egui::bevy_egui::EguiPlugin;
use bevy_inspector_egui::quick::WorldInspectorPlugin;

pub struct ChunkRenderDebug;

impl Plugin for ChunkRenderDebug {
    fn build(&self, app: &mut App) {
        app.add_plugins(DefaultPlugins);
        app.add_plugins(EguiPlugin::default()).add_plugins(WorldInspectorPlugin::new());
        app.add_systems(Startup, setup);
        app.add_systems(FixedUpdate, (on_add_player, on_move));
        app.add_systems(FixedUpdate, on_add_chunk);
    }
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    commands.spawn(Camera2d);
}

fn on_add_chunk(
    chunks: Query<(Entity, &ChunkPos), Added<ChunkStatus>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut commands: Commands,
) {
    chunks.iter().for_each(|(entity, chunk_pos)| {
        if chunk_pos.y != 0 {
            return;
        }
        commands.entity(entity).insert((
            Mesh2d(meshes.add(Rectangle::default())),
            MeshMaterial2d(materials.add(Color::from(GREEN))),
            Transform::from_xyz(chunk_pos.z as f32 * 16.0, chunk_pos.x as f32 * 16.0, 0.0)
                .with_scale(Vec3::splat(16.)),
        ));
    })
}

fn on_add_player(
    players: Query<(Entity, &Position), Added<PlayerChunkObserver>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut commands: Commands,
) {
    players.iter().for_each(|(entity, pos)| {
        commands.entity(entity).insert((
            Mesh2d(meshes.add(Circle::default())),
            MeshMaterial2d(materials.add(Color::from(RED))),
            Transform::from_xyz(pos.z as f32, pos.x as f32, 10.1).with_scale(Vec3::splat(8.0)),
        ));
    })
}

fn on_move(mut players: Query<(&Position, Mut<Transform>)>) {
    players.iter_mut().for_each(|(pos, mut transform)| {
        transform.translation = Vec3::new((pos.z as f32) - 8.0, (pos.x as f32) - 8.0, 10.1);
    })
}

// fn ensure_sent_chunks(
//     mut commands: Commands,
//     q: Query<Entity, (With<PlayerChunkObserver>, Without<SentChunkEntities>)>,
// ) {
//     for entity in &q {
//         commands.entity(entity).insert(SentChunkEntities::default());
//     }
// }
//
// fn spawn_sent_chunks(
//     mut query: Query<(&PlayerChunkObserver, &mut SentChunkEntities)>,
//     chunk_index: Res<ChunkIndex>,
//     mut commands: Commands,
//     mut meshes: ResMut<Assets<Mesh>>,
//     mut materials: ResMut<Assets<ColorMaterial>>,
// ) {
//     query.iter_mut().for_each(|(observer, mut sent_chunks)| {
//         sent_chunks.map.retain(|pos, entity| {
//             let chunk_pos = ChunkPos::new(pos.x, 0, pos.z);
//             if chunk_index.get(&chunk_pos).is_none() || observer.sent_chunks.contains(pos) == false {
//                 commands.entity(*entity).despawn();
//                 return false;
//             }
//             true
//         });
//         observer.sent_chunks.iter().for_each(|pos| {
//             if sent_chunks.map.contains_key(pos) {
//                 return;
//             }
//             let chunk_pos = ChunkPos::new(pos.x, 0, pos.z);
//             if let Some(chunk_entity) = chunk_index.get(&chunk_pos) {
//                 let sent_entity = commands.spawn((
//                     Mesh2d(meshes.add(Rectangle::default())),
//                     MeshMaterial2d(materials.add(Color::from(PURPLE))),
//                     Transform::from_xyz(chunk_pos.z as f32 * 16.0, chunk_pos.x as f32 * 16.0, 1.0).with_scale(Vec3::splat(15.)),
//                 )).id();
//                 sent_chunks.map.insert(*pos, sent_entity);
//             }
//         })
//     })
// }
