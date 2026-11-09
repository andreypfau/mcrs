use alloc::string::String;
use core::fmt::Write;

use mcrs_molang::{BinaryFn, EaseDirection, EaseShape, PureInstr, PureProgram, TernaryFn, UnaryFn};

/// The WGSL functions that every generated channel function calls. A shader module holds it
/// once, before the channel functions.
pub const LIBRARY: &str = include_str!("library.wgsl");

/// Writes `program` as one WGSL function `name`. `name` must be a WGSL identifier that does not
/// start with `molang_`.
///
/// The function takes `opaque_zero: u32`, then one `f32` for each entry of
/// [`PureProgram::inputs`], and returns the channel value limited to `[-1e6, 1e6]`, with `0.0`
/// for a NaN. The caller must pass a zero that the shader compiler cannot see, for example a
/// field of a uniform buffer: it keeps fast math from changing the result bits.
pub fn write_function(out: &mut String, program: &PureProgram, name: &str) {
    debug_assert!(
        !name.is_empty()
            && !name.starts_with("molang_")
            && !name.starts_with(|c: char| c.is_ascii_digit())
            && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'),
        "the function name must be a WGSL identifier that does not start with `molang_`"
    );
    // Writing to a `String` cannot fail.
    let _ = write_body(out, program, name);
}

fn write_body(out: &mut String, program: &PureProgram, name: &str) -> core::fmt::Result {
    write!(out, "fn {name}(opaque_zero: u32")?;
    for index in 0..program.inputs().len() {
        write!(out, ", i{index}: f32")?;
    }
    out.push_str(") -> f32 {\n    molang_zero = opaque_zero;\n");
    for register in 0..program.registers() {
        writeln!(out, "    var r{register}: f32 = 0.0;")?;
    }
    let mut depth = 1usize;
    for instr in program.instructions() {
        if matches!(instr, PureInstr::Else { .. } | PureInstr::EndIf) {
            debug_assert!(depth > 1, "the compiler balances every `else` and `end if`");
            depth = depth.saturating_sub(1);
        }
        for _ in 0..depth {
            out.push_str("    ");
        }
        write_instr(out, *instr)?;
        out.push('\n');
        if matches!(instr, PureInstr::If { .. } | PureInstr::Else { .. }) {
            depth += 1;
        }
    }
    out.push_str("    return 0.0;\n}\n");
    Ok(())
}

/// The WGSL function `name` for `program`; see [`write_function`].
#[must_use]
pub fn lower(program: &PureProgram, name: &str) -> String {
    let mut out = String::new();
    write_function(&mut out, program, name);
    out
}

fn write_instr(out: &mut String, instr: PureInstr) -> core::fmt::Result {
    match instr {
        PureInstr::Constant { dst, value } => {
            write!(
                out,
                "r{} = molang_constant({:#010x}u);",
                dst.index(),
                value.to_bits()
            )
        }
        PureInstr::Input { dst, input } => {
            write!(out, "r{} = molang_flush(i{input});", dst.index())
        }
        PureInstr::Unary { function, dst, a } => {
            write!(
                out,
                "r{} = {}(r{});",
                dst.index(),
                unary_name(function),
                a.index()
            )
        }
        PureInstr::Binary {
            function,
            dst,
            a,
            b,
        } => write!(
            out,
            "r{} = {};",
            dst.index(),
            binary_call(function, a.index(), b.index())
        ),
        PureInstr::Ternary {
            function,
            dst,
            a,
            b,
            c,
        } => {
            let call = ternary_call(function, [a.index(), b.index(), c.index()]);
            write!(out, "r{} = {call};", dst.index())
        }
        PureInstr::Select {
            dst,
            condition,
            then,
            otherwise,
        } => write!(
            out,
            "r{} = select(r{}, r{}, molang_is_truthy(r{}));",
            dst.index(),
            otherwise.index(),
            then.index(),
            condition.index()
        ),
        PureInstr::Move { dst, src } => write!(out, "r{} = r{};", dst.index(), src.index()),
        PureInstr::If { condition, .. } => {
            write!(out, "if (molang_is_truthy(r{})) {{", condition.index())
        }
        PureInstr::Else { .. } => out.write_str("} else {"),
        PureInstr::EndIf => out.write_str("}"),
        PureInstr::Return { src } => write!(out, "return molang_pose_clamp(r{});", src.index()),
    }
}

fn unary_name(function: UnaryFn) -> &'static str {
    match function {
        UnaryFn::Negate => "molang_negate",
        UnaryFn::Not => "molang_not",
        UnaryFn::Truthy => "molang_truthy",
        UnaryFn::Abs => "molang_abs",
        UnaryFn::Ceil => "ceil",
        UnaryFn::Floor => "floor",
        UnaryFn::Trunc => "trunc",
        UnaryFn::Round => "molang_round",
        UnaryFn::Sqrt => "molang_sqrt",
        UnaryFn::Exp => "molang_exp",
        UnaryFn::Ln => "molang_ln",
        UnaryFn::Sin => "molang_sin",
        UnaryFn::Cos => "molang_cos",
        UnaryFn::Asin => "molang_asin",
        UnaryFn::Acos => "molang_acos",
        UnaryFn::Atan => "molang_atan",
        UnaryFn::Sign => "molang_sign",
        UnaryFn::MinAngle => "molang_min_angle",
        UnaryFn::HermiteBlend => "molang_hermite_blend",
    }
}

fn binary_call(function: BinaryFn, a: usize, b: usize) -> String {
    let name = match function {
        BinaryFn::Less => return alloc::format!("molang_number(r{a} < r{b})"),
        BinaryFn::LessEqual => return alloc::format!("molang_number(r{a} <= r{b})"),
        BinaryFn::Greater => return alloc::format!("molang_number(r{a} > r{b})"),
        BinaryFn::GreaterEqual => return alloc::format!("molang_number(r{a} >= r{b})"),
        BinaryFn::Equal => return alloc::format!("molang_number(r{a} == r{b})"),
        BinaryFn::NotEqual => return alloc::format!("molang_number(r{a} != r{b})"),
        BinaryFn::Add => "molang_add",
        BinaryFn::Subtract => "molang_sub",
        BinaryFn::Multiply => "molang_mul",
        BinaryFn::Divide => "molang_divide",
        BinaryFn::BothTruthy => "molang_both_truthy",
        BinaryFn::EitherTruthy => "molang_either_truthy",
        BinaryFn::ApproxEqual => "molang_approx_eq",
        BinaryFn::Min => "molang_min",
        BinaryFn::Max => "molang_max",
        BinaryFn::Mod => "molang_mod",
        BinaryFn::Pow => "molang_pow",
        BinaryFn::Atan2 => "molang_atan2",
        BinaryFn::CopySign => "molang_copy_sign",
    };
    alloc::format!("{name}(r{a}, r{b})")
}

fn ternary_call(function: TernaryFn, [a, b, c]: [usize; 3]) -> String {
    let name = match function {
        TernaryFn::Clamp => "molang_clamp",
        TernaryFn::Lerp => "molang_lerp",
        TernaryFn::InverseLerp => "molang_inverse_lerp",
        TernaryFn::LerpRotate => "molang_lerp_rotate",
        TernaryFn::InRange => "molang_in_range",
        TernaryFn::Ease(easing) => {
            let direction = match easing.direction {
                EaseDirection::In => 0,
                EaseDirection::Out => 1,
                EaseDirection::InOut => 2,
            };
            let shape = match easing.shape {
                EaseShape::Quad => 0,
                EaseShape::Cubic => 1,
                EaseShape::Quart => 2,
                EaseShape::Quint => 3,
                EaseShape::Sine => 4,
                EaseShape::Expo => 5,
                EaseShape::Circ => 6,
                EaseShape::Back => 7,
                EaseShape::Elastic => 8,
                EaseShape::Bounce => 9,
            };
            return alloc::format!("molang_ease({direction}u, {shape}u, r{a}, r{b}, r{c})");
        }
    };
    alloc::format!("{name}(r{a}, r{b}, r{c})")
}
