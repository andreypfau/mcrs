//! The entity queries of `docs/spec/molang-queries.md`, so that a test sees the errors of the
//! language rather than unknown names. The `spec` test keeps the table equal to the document.

use mcrs_molang::{Arguments, QueryDeclaration, QueryRegistry, Rate, UseContext, ValueType};

/// How a query reaches a GPU channel: the column "Pose input" of the spec.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PoseInput {
    /// `time`: a value of the entity clock, never negative.
    Time,
    /// `tick` for a flag: `0` or `1`.
    Flag,
    /// Any other pose input kind.
    Other,
    /// `—`: not a pose input.
    None,
}

pub struct EntityQuery {
    pub name: &'static str,
    pub result: ValueType,
    pub arguments: u8,
    pub rate: Rate,
    pub pose: PoseInput,
}

const fn query(
    name: &'static str,
    result: ValueType,
    arguments: u8,
    rate: Rate,
    pose: PoseInput,
) -> EntityQuery {
    EntityQuery {
        name,
        result,
        arguments,
        rate,
        pose,
    }
}

pub const ENTITY_QUERIES: [EntityQuery; 37] = [
    query(
        "life_time",
        ValueType::Number,
        0,
        Rate::EntityFrame,
        PoseInput::Time,
    ),
    query(
        "delta_time",
        ValueType::Number,
        0,
        Rate::EntityFrame,
        PoseInput::Time,
    ),
    query(
        "frame_alpha",
        ValueType::Number,
        0,
        Rate::Frame,
        PoseInput::Other,
    ),
    query(
        "time_of_day",
        ValueType::Number,
        0,
        Rate::Frame,
        PoseInput::Other,
    ),
    query(
        "modified_distance_moved",
        ValueType::Number,
        0,
        Rate::EntityFrame,
        PoseInput::Other,
    ),
    query(
        "modified_move_speed",
        ValueType::Number,
        0,
        Rate::EntityFrame,
        PoseInput::Other,
    ),
    query(
        "ground_speed",
        ValueType::Number,
        0,
        Rate::EntityTick,
        PoseInput::Other,
    ),
    query(
        "vertical_speed",
        ValueType::Number,
        0,
        Rate::EntityTick,
        PoseInput::Other,
    ),
    query(
        "is_moving",
        ValueType::Number,
        0,
        Rate::EntityTick,
        PoseInput::Flag,
    ),
    query(
        "is_on_ground",
        ValueType::Number,
        0,
        Rate::EntityTick,
        PoseInput::Flag,
    ),
    query(
        "is_in_water",
        ValueType::Number,
        0,
        Rate::EntityTick,
        PoseInput::Flag,
    ),
    query(
        "target_x_rotation",
        ValueType::Number,
        0,
        Rate::EntityFrame,
        PoseInput::Other,
    ),
    query(
        "target_y_rotation",
        ValueType::Number,
        0,
        Rate::EntityFrame,
        PoseInput::Other,
    ),
    query(
        "body_y_rotation",
        ValueType::Number,
        0,
        Rate::EntityFrame,
        PoseInput::Other,
    ),
    query(
        "head_x_rotation",
        ValueType::Number,
        1,
        Rate::EntityFrame,
        PoseInput::Other,
    ),
    query(
        "head_y_rotation",
        ValueType::Number,
        1,
        Rate::EntityFrame,
        PoseInput::Other,
    ),
    query(
        "is_baby",
        ValueType::Number,
        0,
        Rate::EntityTick,
        PoseInput::Flag,
    ),
    query(
        "is_alive",
        ValueType::Number,
        0,
        Rate::EntityTick,
        PoseInput::Flag,
    ),
    query(
        "hurt_time",
        ValueType::Number,
        0,
        Rate::EntityTick,
        PoseInput::Other,
    ),
    query(
        "death_ticks",
        ValueType::Number,
        0,
        Rate::EntityFrame,
        PoseInput::Other,
    ),
    query(
        "variant",
        ValueType::Number,
        0,
        Rate::EntityTick,
        PoseInput::Other,
    ),
    query(
        "variant_identifier",
        ValueType::String,
        0,
        Rate::EntityTick,
        PoseInput::None,
    ),
    query(
        "model_scale",
        ValueType::Number,
        0,
        Rate::EntityTick,
        PoseInput::Other,
    ),
    query(
        "is_saddled",
        ValueType::Number,
        0,
        Rate::EntityTick,
        PoseInput::Flag,
    ),
    query(
        "pose",
        ValueType::String,
        0,
        Rate::EntityTick,
        PoseInput::None,
    ),
    query(
        "is_on_fire",
        ValueType::Number,
        0,
        Rate::EntityTick,
        PoseInput::Flag,
    ),
    query(
        "is_sneaking",
        ValueType::Number,
        0,
        Rate::EntityTick,
        PoseInput::Flag,
    ),
    query(
        "is_sprinting",
        ValueType::Number,
        0,
        Rate::EntityTick,
        PoseInput::Flag,
    ),
    query(
        "is_swimming",
        ValueType::Number,
        0,
        Rate::EntityTick,
        PoseInput::Flag,
    ),
    query(
        "is_invisible",
        ValueType::Number,
        0,
        Rate::EntityTick,
        PoseInput::Flag,
    ),
    query(
        "is_riding",
        ValueType::Number,
        0,
        Rate::EntityTick,
        PoseInput::Flag,
    ),
    query(
        "has_rider",
        ValueType::Number,
        0,
        Rate::EntityTick,
        PoseInput::Flag,
    ),
    query(
        "anim_time",
        ValueType::Number,
        0,
        Rate::AnimationFrame,
        PoseInput::Time,
    ),
    query(
        "key_frame_lerp_time",
        ValueType::Number,
        0,
        Rate::AnimationFrame,
        PoseInput::Time,
    ),
    query(
        "all_animations_finished",
        ValueType::Number,
        0,
        Rate::AnimationFrame,
        PoseInput::None,
    ),
    query(
        "any_animation_finished",
        ValueType::Number,
        0,
        Rate::AnimationFrame,
        PoseInput::None,
    ),
    query(
        "distance_from_camera",
        ValueType::Number,
        0,
        Rate::EntityFrame,
        PoseInput::None,
    ),
];

pub fn registry() -> QueryRegistry {
    let mut registry = QueryRegistry::new(UseContext::Entity);
    for query in &ENTITY_QUERIES {
        let arguments = match query.arguments {
            0 => Arguments::None,
            count => Arguments::Literals(count),
        };
        registry.declare(QueryDeclaration::entity(
            query.name,
            query.result,
            arguments,
            query.rate,
        ));
    }
    registry
}
