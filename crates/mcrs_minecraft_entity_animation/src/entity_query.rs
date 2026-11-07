use mcrs_molang::{
    Arguments, LiteralKey, Milestone, QueryDeclaration, QueryRegistry, QueryScope, Rate,
    UseContext, ValueType,
};

/// A query of the context of use `entity` that the animation host supplies.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum EntityQuery {
    LifeTime,
    DeltaTime,
    FrameAlpha,
    TimeOfDay,
    ModifiedDistanceMoved,
    ModifiedMoveSpeed,
    GroundSpeed,
    VerticalSpeed,
    IsMoving,
    IsOnGround,
    IsInWater,
    TargetXRotation,
    TargetYRotation,
    BodyYRotation,
    /// The head index of the argument; `None` for an argument that is not a small whole number.
    HeadXRotation(Option<u8>),
    HeadYRotation(Option<u8>),
    IsBaby,
    IsAlive,
    HurtTime,
    DeathTicks,
    /// The position of the variant entry in the synced registry.
    Variant,
    /// The resource location of the variant entry, as a string.
    VariantIdentifier,
    VariantAsset,
    VariantModel,
    ModelScale,
    IsSaddled,
    Pose,
    IsOnFire,
    IsSneaking,
    IsSprinting,
    IsSwimming,
    IsInvisible,
    IsRiding,
    HasRider,
    AnimTime,
    KeyFrameLerpTime,
    AllAnimationsFinished,
    AnyAnimationFinished,
}

struct Row {
    name: &'static str,
    query: EntityQuery,
    result: ValueType,
    rate: Rate,
    scope: QueryScope,
    milestone: Milestone,
}

const fn row(name: &'static str, query: EntityQuery, rate: Rate) -> Row {
    Row {
        name,
        query,
        result: ValueType::Number,
        rate,
        scope: QueryScope::Entity,
        milestone: Milestone::One,
    }
}

const fn text(name: &'static str, query: EntityQuery) -> Row {
    Row {
        result: ValueType::String,
        ..row(name, query, Rate::EntityTick)
    }
}

const fn any(name: &'static str, query: EntityQuery, rate: Rate) -> Row {
    Row {
        scope: QueryScope::Any,
        ..row(name, query, rate)
    }
}

const fn later(row: Row, milestone: Milestone) -> Row {
    Row { milestone, ..row }
}

use EntityQuery as Q;
use Rate::{AnimationFrame, EntityFrame, EntityTick, Frame};

const ROWS: [Row; 38] = [
    row("life_time", Q::LifeTime, EntityFrame),
    any("delta_time", Q::DeltaTime, EntityFrame),
    any("frame_alpha", Q::FrameAlpha, Frame),
    any("time_of_day", Q::TimeOfDay, Frame),
    row(
        "modified_distance_moved",
        Q::ModifiedDistanceMoved,
        EntityFrame,
    ),
    row("modified_move_speed", Q::ModifiedMoveSpeed, EntityFrame),
    later(
        row("ground_speed", Q::GroundSpeed, EntityTick),
        Milestone::Two,
    ),
    later(
        row("vertical_speed", Q::VerticalSpeed, EntityTick),
        Milestone::Two,
    ),
    later(row("is_moving", Q::IsMoving, EntityTick), Milestone::Two),
    later(
        row("is_on_ground", Q::IsOnGround, EntityTick),
        Milestone::Two,
    ),
    later(row("is_in_water", Q::IsInWater, EntityTick), Milestone::Two),
    row("target_x_rotation", Q::TargetXRotation, EntityFrame),
    row("target_y_rotation", Q::TargetYRotation, EntityFrame),
    row("body_y_rotation", Q::BodyYRotation, EntityFrame),
    row("head_x_rotation", Q::HeadXRotation(None), EntityFrame),
    row("head_y_rotation", Q::HeadYRotation(None), EntityFrame),
    row("is_baby", Q::IsBaby, EntityTick),
    row("is_alive", Q::IsAlive, EntityTick),
    row("hurt_time", Q::HurtTime, EntityTick),
    row("death_ticks", Q::DeathTicks, EntityFrame),
    row("variant", Q::Variant, EntityTick),
    text("variant_identifier", Q::VariantIdentifier),
    text("variant_asset", Q::VariantAsset),
    text("variant_model", Q::VariantModel),
    row("model_scale", Q::ModelScale, EntityTick),
    row("is_saddled", Q::IsSaddled, EntityTick),
    text("pose", Q::Pose),
    row("is_on_fire", Q::IsOnFire, EntityTick),
    row("is_sneaking", Q::IsSneaking, EntityTick),
    row("is_sprinting", Q::IsSprinting, EntityTick),
    row("is_swimming", Q::IsSwimming, EntityTick),
    row("is_invisible", Q::IsInvisible, EntityTick),
    row("is_riding", Q::IsRiding, EntityTick),
    row("has_rider", Q::HasRider, EntityTick),
    row("anim_time", Q::AnimTime, AnimationFrame),
    row("key_frame_lerp_time", Q::KeyFrameLerpTime, AnimationFrame),
    row(
        "all_animations_finished",
        Q::AllAnimationsFinished,
        AnimationFrame,
    ),
    row(
        "any_animation_finished",
        Q::AnyAnimationFinished,
        AnimationFrame,
    ),
];

impl EntityQuery {
    /// The name after `query.`.
    #[must_use]
    pub fn name(self) -> &'static str {
        let base = match self {
            Self::HeadXRotation(_) => Self::HeadXRotation(None),
            Self::HeadYRotation(_) => Self::HeadYRotation(None),
            other => other,
        };
        ROWS.iter()
            .find(|row| row.query == base)
            .map_or("", |row| row.name)
    }

    /// A registry of the context of use `entity` with each entity query declared.
    #[must_use]
    pub fn registry() -> QueryRegistry {
        let mut registry = QueryRegistry::new(UseContext::Entity);
        for row in &ROWS {
            let arguments = match row.query {
                Self::HeadXRotation(_) | Self::HeadYRotation(_) => Arguments::Literals(1),
                _ => Arguments::None,
            };
            registry.declare(QueryDeclaration {
                name: row.name.into(),
                arguments,
                result: row.result,
                rate: row.rate,
                scope: row.scope,
                milestone: row.milestone,
            });
        }
        registry
    }

    /// The query of a declaration name with the literal arguments of one query slot.
    #[must_use]
    pub fn from_declaration(name: &str, arguments: &[LiteralKey]) -> Option<Self> {
        let query = ROWS.iter().find(|row| row.name == name)?.query;
        let head = || match arguments.first() {
            Some(LiteralKey::Number(bits)) => {
                let value = f32::from_bits(*bits);
                (value.fract() == 0.0 && (0.0..=255.0).contains(&value)).then_some(value as u8)
            }
            _ => None,
        };
        Some(match query {
            Self::HeadXRotation(_) => Self::HeadXRotation(head()),
            Self::HeadYRotation(_) => Self::HeadYRotation(head()),
            other => other,
        })
    }

    /// Whether the animation host supplies the value itself, not the query source.
    #[must_use]
    pub const fn is_animation_supplied(self) -> bool {
        matches!(
            self,
            Self::DeltaTime
                | Self::AnimTime
                | Self::KeyFrameLerpTime
                | Self::AllAnimationsFinished
                | Self::AnyAnimationFinished
        )
    }
}
