use std::time::Duration;

use bevy::prelude::*;
use mcrs_minecraft_client_world::joined_game::CurrentDimension;
use mcrs_minecraft_dimension::Dimension;
use mcrs_minecraft_fog::{DrawnRings, follow_fog_end};
use mcrs_minecraft_render::RenderDistance;
use mcrs_minecraft_resource::ResourceKeyBuf;
use mcrs_voxel::ColumnPos;

use crate::camera::PlayerCamera;
use crate::drawn_columns::DrawnColumn;
use crate::stream::Loader;

const TICK_WITHOUT_FIXED_TIME: Duration = Duration::from_millis(50);

/// The end of the render-distance fog: the rings of drawn columns around the column of the
/// camera, materialized by `track_drawn_columns`, and the end the frames move toward their
/// target, kept between frames by `follow_render_fog`.
#[derive(Resource, Default)]
pub(crate) struct RenderFog {
    rings: DrawnRings,
    centre: Option<ColumnPos>,
    end: f32,
}

impl RenderFog {
    pub(crate) fn end(&self) -> f32 {
        self.end
    }
}

/// Follows the drawn columns one by one; a move of the camera into another column or a new
/// render distance counts every drawn column again.
pub(crate) fn track_drawn_columns(
    loader: Res<Loader>,
    render_distance: Res<RenderDistance>,
    camera: Option<Single<&GlobalTransform, With<PlayerCamera>>>,
    mut changes: MessageReader<DrawnColumn>,
    mut fog: ResMut<RenderFog>,
) {
    let centre = camera.map(|camera| ColumnPos::from(camera.translation().as_dvec3()));
    if centre != fog.centre || render_distance.0 != fog.rings.render_distance() {
        changes.clear();
        let fog = &mut *fog;
        fog.centre = centre;
        fog.rings.set_render_distance(render_distance.0);
        if let Some(centre) = centre {
            for pos in loader.drawn_columns() {
                fog.rings.insert(pos.x - centre.x, pos.z - centre.z);
            }
        }
        return;
    }
    let Some(centre) = centre else {
        changes.clear();
        return;
    };
    for change in changes.read() {
        match *change {
            DrawnColumn::Drawn(pos) => fog.rings.insert(pos.x - centre.x, pos.z - centre.z),
            DrawnColumn::Hidden(pos) => fog.rings.remove(pos.x - centre.x, pos.z - centre.z),
        }
    }
}

/// Moves the end toward the target of the rings by the ticks since the previous frame. It
/// starts from 0 in each world and each dimension the player enters.
pub(crate) fn follow_render_fog(
    time: Res<Time>,
    fixed: Option<Res<Time<Fixed>>>,
    current: Option<Single<&CurrentDimension>>,
    mut entered: Local<Option<ResourceKeyBuf<Dimension>>>,
    fog: ResMut<RenderFog>,
) {
    let current = current.as_deref().map(|current| &current.key);
    let previous = if current == entered.as_ref() {
        fog.end
    } else {
        *entered = current.cloned();
        0.0
    };
    let tick = fixed.map_or(TICK_WITHOUT_FIXED_TIME, |fixed| fixed.timestep());
    let ticks = time.delta().as_secs_f32() / tick.as_secs_f32().max(f32::EPSILON);
    let end = follow_fog_end(previous, fog.rings.fog_end_target(), ticks);
    fog.map_unchanged(|fog| &mut fog.end).set_if_neq(end);
}
