use bevy_ecs::prelude::*;
use glam::{DVec2, DVec3};
use mcrs_minecraft_client_world::columns::ColumnStore;
use mcrs_minecraft_entity::Life;
use mcrs_minecraft_protocol_game::ColumnPos;

use crate::angle;
use crate::body_turn;
use crate::client_entity::ClientEntity;
use crate::living::Living;
use crate::motion::{Age, Motion};
use crate::server_ticking::ServerTicking;
use crate::synced_data::SyncedData;
use crate::walk::{self, BABY_WALK_SCALE};

/// One tick of every client entity that ticks.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct ClientEntityTick;

/// Whether an entity at `position` is in the ticking area: its column is loaded. A client with
/// no column store ticks every entity.
#[must_use]
pub fn in_ticking_area(columns: Option<&ColumnStore>, position: DVec3) -> bool {
    let column = ColumnPos::new(
        (position.x.floor() as i32) >> 4,
        (position.z.floor() as i32) >> 4,
    );
    columns.is_none_or(|columns| columns.holds(column))
}

/// Runs one tick of one entity, in the order of the tick of a living entity.
pub fn tick_entity(
    entity: &ClientEntity,
    motion: &mut Motion,
    age: &mut Age,
    living: Option<&mut Living>,
    data: &SyncedData,
    tick_speed: f32,
) {
    motion.previous = motion.current;
    motion.current = motion.interpolation.step(motion.current, tick_speed);
    age.0 = age.0.saturating_add(1);
    if let Some(living) = living {
        tick_living(entity, motion, living, data);
    }
    motion.previous.yaw = angle::unwrap_towards(motion.previous.yaw, motion.current.yaw);
}

fn tick_living(entity: &ClientEntity, motion: &Motion, living: &mut Living, data: &SyncedData) {
    let Some(type_data) = entity.type_data() else {
        return;
    };
    living.hurt_time = living.hurt_time.saturating_sub(1);
    if data.is_dying() {
        living.death_time = living.death_time.saturating_add(1);
    }
    living.previous_head_yaw = living.head_yaw;
    living.previous_body_yaw = living.body_yaw;
    living.head_yaw = living.head_turn.step(living.head_yaw);

    let movement = motion.movement();
    let horizontal = DVec2::new(movement.x, movement.z);
    if data.is_dying() {
        living.walk.stop();
    } else {
        let factor = walk::target_factor(type_data, data.pose);
        let target = walk::target_speed(horizontal.length() as f32, factor);
        let scale = if data.baby { BABY_WALK_SCALE } else { 1.0 };
        living.walk.advance(target, scale);
    }

    match type_data.life {
        Life::Mob(mob) => {
            let turn = living.body_turn.follow(
                motion.current.yaw,
                living.head_yaw,
                living.body_yaw,
                horizontal,
                mob.head_yaw_limit,
            );
            living.body_yaw = turn.body_yaw;
            living.head_yaw = turn.head_yaw;
        }
        Life::Living => {
            living.body_yaw =
                body_turn::follow_movement(motion.current.yaw, living.body_yaw, horizontal);
        }
        _ => {}
    }

    living.previous_head_yaw = angle::unwrap_towards(living.previous_head_yaw, living.head_yaw);
    living.previous_body_yaw = angle::unwrap_towards(living.previous_body_yaw, living.body_yaw);
}

pub(crate) fn tick_entities(
    columns: Option<Res<ColumnStore>>,
    ticking: Res<ServerTicking>,
    mut entities: Query<(
        &ClientEntity,
        &mut Motion,
        &mut Age,
        Option<&mut Living>,
        &SyncedData,
    )>,
) {
    for (entity, mut motion, mut age, living, data) in &mut entities {
        if !in_ticking_area(columns.as_deref(), motion.current.position) {
            continue;
        }
        tick_entity(
            entity,
            &mut motion,
            &mut age,
            living.map(Mut::into_inner),
            data,
            ticking.tick_speed(),
        );
    }
}

pub(crate) fn run_world_ticks(ticking: Res<ServerTicking>) -> bool {
    ticking.runs()
}

pub(crate) fn consume_step(mut ticking: ResMut<ServerTicking>) {
    if ticking.frozen && ticking.steps > 0 {
        ticking.steps -= 1;
    }
}
