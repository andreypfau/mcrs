/// The arity of a `math.` name in MOL-MATH-002, or `None` when MOL does not list the name.
/// `Some(0)` is the constant `math.pi`, which has no argument list.
pub fn spec_arity(name: &str) -> Option<usize> {
    Some(match name {
        "pi" => 0,
        "abs" | "ceil" | "floor" | "trunc" | "round" | "sqrt" | "exp" | "ln" | "sin" | "cos"
        | "asin" | "acos" | "atan" | "sign" | "min_angle" | "hermite_blend" => 1,
        "pow" | "atan2" | "min" | "max" | "mod" | "copy_sign" | "random" | "random_integer" => 2,
        "clamp" | "lerp" | "inverse_lerp" | "lerprotate" | "die_roll" | "die_roll_integer" => 3,
        _ if is_easing(name) => 3,
        _ => return None,
    })
}

fn is_easing(name: &str) -> bool {
    const SHAPES: [&str; 10] = [
        "quad", "cubic", "quart", "quint", "sine", "expo", "circ", "back", "elastic", "bounce",
    ];
    let Some(rest) = name.strip_prefix("ease_") else {
        return false;
    };
    ["in_out_", "in_", "out_"]
        .iter()
        .filter_map(|direction| rest.strip_prefix(direction))
        .any(|shape| SHAPES.contains(&shape))
}
