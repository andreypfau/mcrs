use mcrs_minecraft_entity_attribute::attribute_keys::Attribute;
use mcrs_minecraft_physics::MotionAttributes;

pub(crate) fn motion_field(attribute: Attribute) -> Option<fn(&mut MotionAttributes) -> &mut f64> {
    let field: fn(&mut MotionAttributes) -> &mut f64 = match attribute {
        Attribute::Gravity => |attributes| &mut attributes.gravity,
        Attribute::StepHeight => |attributes| &mut attributes.step_height,
        Attribute::MovementSpeed => |attributes| &mut attributes.movement_speed,
        Attribute::JumpStrength => |attributes| &mut attributes.jump_strength,
        Attribute::Scale => |attributes| &mut attributes.scale,
        Attribute::SneakingSpeed => |attributes| &mut attributes.sneaking_speed,
        Attribute::MovementEfficiency => |attributes| &mut attributes.movement_efficiency,
        Attribute::Bounciness => |attributes| &mut attributes.bounciness,
        Attribute::FrictionModifier => |attributes| &mut attributes.friction_modifier,
        Attribute::AirDragModifier => |attributes| &mut attributes.air_drag_modifier,
        _ => return None,
    };
    Some(field)
}
