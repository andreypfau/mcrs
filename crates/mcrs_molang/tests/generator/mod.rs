//! The grammar-based generator of Molang programs that the differential tests share.

use mcrs_molang::{Arguments, QueryDeclaration, QueryRegistry, Rate, UseContext, ValueType};
use proptest::prelude::*;

pub const VARIABLES: [&str; 4] = ["a", "b", "c", "d"];
pub const NUMBER_QUERIES: [&str; 2] = ["qa", "qb"];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Profile {
    /// Every construct that the compiler implements, with non-finite and signed-zero inputs.
    Full,
    /// The subset where molangjs agrees with the language, after the normalisations of
    /// `tools/molang-diff/README.md`, section 3.
    MolangJs,
    /// The constructs of a channel that can run on a GPU: no write, no string, no `??` and no
    /// unknown query. Some calls keep their arguments in the domain of the function.
    Gpu,
}

fn literal(profile: Profile) -> BoxedStrategy<String> {
    let full = [
        "0", "1", "2", "0.5", "3.25", "90", "180", "-0", "1000000", ".5", "2f", "0.1", "16777217",
        "true", "false",
    ];
    let js = ["0", "1", "2", "3", "0.5", "2.5", "10", "true", "false"];
    let pool: Vec<&'static str> = match profile {
        Profile::Full | Profile::Gpu => full.to_vec(),
        Profile::MolangJs => js.to_vec(),
    };
    proptest::sample::select(pool)
        .prop_map(|text| format!("({text})"))
        .boxed()
}

fn leaf(profile: Profile) -> BoxedStrategy<String> {
    let reads = prop_oneof![
        proptest::sample::select(VARIABLES.to_vec()).prop_map(|name| format!("v.{name}")),
        proptest::sample::select(vec!["t.x", "t.y"]).prop_map(str::to_owned),
        proptest::sample::select(NUMBER_QUERIES.to_vec()).prop_map(|name| format!("q.{name}")),
        Just("this".to_owned()),
    ];
    match profile {
        Profile::Full => prop_oneof![
            3 => literal(profile),
            3 => reads,
            1 => Just("q.unknown".to_owned()),
            1 => Just("math.pi".to_owned()),
        ]
        .boxed(),
        Profile::MolangJs => prop_oneof![3 => literal(profile), 3 => reads].boxed(),
        Profile::Gpu => prop_oneof![
            3 => literal(profile),
            3 => reads,
            1 => Just("math.pi".to_owned()),
        ]
        .boxed(),
    }
}

fn unary_functions(profile: Profile) -> Vec<&'static str> {
    match profile {
        Profile::Full | Profile::Gpu => vec![
            "abs",
            "ceil",
            "floor",
            "trunc",
            "round",
            "sqrt",
            "exp",
            "ln",
            "sin",
            "cos",
            "asin",
            "acos",
            "atan",
            "sign",
            "min_angle",
            "hermite_blend",
        ],
        Profile::MolangJs => vec!["abs", "ceil", "floor", "trunc", "hermite_blend"],
    }
}

fn binary_functions(profile: Profile) -> Vec<&'static str> {
    match profile {
        Profile::Full | Profile::Gpu => vec!["min", "max", "mod", "pow", "atan2", "copy_sign"],
        Profile::MolangJs => vec!["min", "max"],
    }
}

fn ternary_functions(profile: Profile) -> Vec<&'static str> {
    match profile {
        Profile::Full | Profile::Gpu => vec![
            "clamp",
            "lerp",
            "inverse_lerp",
            "lerprotate",
            "ease_in_quad",
            "ease_out_back",
            "ease_in_out_bounce",
            "ease_in_out_elastic",
        ],
        Profile::MolangJs => vec!["lerp", "ease_in_quad", "ease_out_cubic"],
    }
}

fn operators(profile: Profile) -> Vec<&'static str> {
    let mut all = vec!["+", "-", "*", "<", "<=", ">", ">=", "==", "!=", "&&", "||"];
    if profile != Profile::MolangJs {
        all.push("/");
    }
    all
}

fn expression(profile: Profile) -> BoxedStrategy<String> {
    leaf(profile)
        .prop_recursive(5, 40, 3, move |inner| {
            let unary = (
                proptest::sample::select(unary_functions(profile)),
                inner.clone(),
            )
                .prop_map(|(name, a)| format!("math.{name}({a})"));
            let binary_call = (
                proptest::sample::select(binary_functions(profile)),
                inner.clone(),
                inner.clone(),
            )
                .prop_map(|(name, a, b)| format!("math.{name}({a}, {b})"));
            let ternary_call = (
                proptest::sample::select(ternary_functions(profile)),
                inner.clone(),
                inner.clone(),
                inner.clone(),
            )
                .prop_map(|(name, a, b, c)| format!("math.{name}({a}, {b}, {c})"));
            let clamp = (inner.clone(), 0u8..4, 0u8..4)
                .prop_map(|(a, lo, width)| format!("math.clamp({a}, {lo}, {})", lo + width));
            let operator = (
                inner.clone(),
                proptest::sample::select(operators(profile)),
                inner.clone(),
            )
                .prop_map(|(a, op, b)| format!("({a} {op} {b})"));
            // molangjs negates as `0 - a`, which loses `-0`.
            let prefixes = match profile {
                Profile::Full | Profile::Gpu => vec!["-", "!"],
                Profile::MolangJs => vec!["!"],
            };
            let prefix = (proptest::sample::select(prefixes), inner.clone())
                .prop_map(|(op, a)| format!("({op}{a})"));
            let conditional = (inner.clone(), inner.clone(), inner.clone())
                .prop_map(|(c, a, b)| format!("({c} ? {a} : {b})"));
            let binary_conditional =
                (inner.clone(), inner.clone()).prop_map(|(c, a)| format!("({c} ? {a})"));
            // molangjs keeps one missing flag for the run, so an inner `??` does not hide a
            // missing read from an outer `??`. Its left side is a plain read here.
            let coalesce = match profile {
                Profile::Full | Profile::Gpu => (inner.clone(), inner.clone())
                    .prop_map(|(a, b)| format!("({a} ?? {b})"))
                    .boxed(),
                Profile::MolangJs => (leaf(profile), inner.clone())
                    .prop_map(|(a, b)| format!("({a} ?? {b})"))
                    .boxed(),
            };
            let assign_group = (
                inner.clone(),
                proptest::sample::select(VARIABLES.to_vec()),
                inner.clone(),
            )
                .prop_map(|(c, name, a)| format!("({c} ? (v.{name} = ({a}) * 1))"));
            if profile == Profile::Gpu {
                return prop_oneof![
                    4 => operator,
                    2 => prefix,
                    2 => conditional,
                    1 => binary_conditional,
                    2 => unary,
                    1 => binary_call,
                    1 => ternary_call,
                    1 => clamp,
                    2 => in_domain(inner.clone()),
                    1 => pure_queries(inner),
                ]
                .boxed();
            }
            let common = prop_oneof![
                4 => operator,
                2 => prefix,
                2 => conditional,
                1 => binary_conditional,
                2 => coalesce,
                2 => unary,
                1 => binary_call,
                1 => ternary_call,
                1 => clamp,
                1 => assign_group,
            ];
            match profile {
                Profile::Full => prop_oneof![8 => common, 1 => full_only(inner)].boxed(),
                Profile::MolangJs | Profile::Gpu => common.boxed(),
            }
        })
        .boxed()
}

/// Calls whose argument stays in the domain where the result is finite, so that a channel with
/// them can still run on a GPU.
fn in_domain(inner: BoxedStrategy<String>) -> BoxedStrategy<String> {
    let one = (
        proptest::sample::select(vec![
            "math.sqrt(math.abs({a}))",
            "math.ln(math.abs({a}) + 0.25)",
            "math.exp(math.clamp({a}, -60, 60))",
            "math.asin(math.clamp({a}, -1, 1))",
            "math.acos(math.clamp({a}, -1, 1))",
            "math.ease_in_out_circ(0, 1, math.clamp({a}, 0, 1))",
            "math.ease_out_expo(0, 1, math.clamp({a}, -2, 2))",
        ]),
        inner.clone(),
    )
        .prop_map(|(form, a)| form.replace("{a}", &a));
    let pow = (inner.clone(), inner)
        .prop_map(|(a, b)| format!("math.pow(math.clamp({a}, 0.5, 4), math.clamp({b}, -8, 8))"));
    prop_oneof![3 => one, 1 => pow].boxed()
}

fn pure_queries(inner: BoxedStrategy<String>) -> BoxedStrategy<String> {
    let pure_query = (
        proptest::sample::select(vec!["in_range", "all", "any"]),
        inner.clone(),
        inner.clone(),
        inner.clone(),
    )
        .prop_map(|(name, a, b, c)| format!("q.{name}({a}, {b}, {c})"));
    let approx = (inner.clone(), inner).prop_map(|(a, b)| format!("q.approx_eq({a}, {b})"));
    prop_oneof![pure_query, approx].boxed()
}

/// The constructs that molangjs does not share with the language: pure queries, strings,
/// blocks as branches and assignments of a missing value.
fn full_only(inner: BoxedStrategy<String>) -> BoxedStrategy<String> {
    let pure_query = (
        proptest::sample::select(vec!["in_range", "all", "any"]),
        inner.clone(),
        inner.clone(),
        inner.clone(),
    )
        .prop_map(|(name, a, b, c)| format!("q.{name}({a}, {b}, {c})"));
    let approx = (inner.clone(), inner.clone()).prop_map(|(a, b)| format!("q.approx_eq({a}, {b})"));
    let strings = (
        proptest::sample::select(vec!["q.sq", "'x'", "'Y'", "''"]),
        prop_oneof![Just("=="), Just("!=")],
        proptest::sample::select(vec!["q.sq", "'x'", "'Y'"]),
    )
        .prop_map(|(a, op, b)| format!("({a} {op} {b})"));
    let block = (inner.clone(), inner.clone())
        .prop_map(|(c, a)| format!("({c} ? {{ t.y = {a}; v.d = t.y; }})"));
    let raw_assign = (
        inner.clone(),
        proptest::sample::select(VARIABLES.to_vec()),
        inner,
    )
        .prop_map(|(c, name, a)| format!("({c} ? (v.{name} = {a}) : (t.x = {a}))"));
    prop_oneof![pure_query, approx, strings, block, raw_assign].boxed()
}

fn statement(profile: Profile) -> BoxedStrategy<String> {
    let assign = (
        proptest::sample::select(VARIABLES.to_vec()),
        expression(profile),
    )
        .prop_map(|(name, value)| format!("v.{name} = ({value}) * 1;"));
    let temp = (
        proptest::sample::select(vec!["x", "y"]),
        expression(profile),
    )
        .prop_map(|(name, value)| format!("t.{name} = ({value}) * 1;"));
    let guarded = (
        expression(profile),
        proptest::sample::select(VARIABLES.to_vec()),
        expression(profile),
    )
        .prop_map(|(c, name, value)| format!("({c}) ? {{ v.{name} = ({value}) * 1; }};"));
    match profile {
        Profile::MolangJs => prop_oneof![assign, temp, guarded].boxed(),
        Profile::Gpu => {
            let early = (expression(profile), expression(profile))
                .prop_map(|(c, value)| format!("({c}) ? {{ return {value}; }};"));
            prop_oneof![2 => temp, 1 => early].boxed()
        }
        Profile::Full => {
            let early = (expression(profile), expression(profile))
                .prop_map(|(c, value)| format!("({c}) ? {{ return {value}; }};"));
            let bare = (
                proptest::sample::select(VARIABLES.to_vec()),
                expression(profile),
            )
                .prop_map(|(name, value)| format!("v.{name} = {value};"));
            prop_oneof![assign, temp, guarded, early, bare].boxed()
        }
    }
}

pub fn program(profile: Profile) -> BoxedStrategy<String> {
    let complex = (
        proptest::collection::vec(statement(profile), 0..4),
        expression(profile),
    )
        .prop_map(|(statements, result)| format!("{} return {result};", statements.concat()));
    let simple = match profile {
        Profile::Full | Profile::Gpu => expression(profile),
        // f64 and f32 trigonometry differ in the last bits, so a comparison of a
        // result can flip; trigonometry appears here only where nothing compares its result.
        Profile::MolangJs => prop_oneof![
            3 => expression(profile),
            1 => (
                proptest::sample::select(vec!["sin", "cos"]),
                expression(profile),
                expression(profile),
            )
                .prop_map(|(name, a, b)| format!("math.{name}(math.clamp({a}, -1000, 1000)) * {b}")),
            1 => (expression(profile), expression(profile))
                .prop_map(|(a, b)| format!("math.ease_in_out_sine(0, {a}, math.clamp({b}, 0, 1))")),
        ]
        .boxed(),
    };
    prop_oneof![1 => simple, 2 => complex].boxed()
}

pub fn registry() -> QueryRegistry {
    let mut registry = QueryRegistry::new(UseContext::Entity);
    for name in NUMBER_QUERIES {
        registry.declare(QueryDeclaration::entity(
            name,
            ValueType::Number,
            Arguments::None,
            Rate::EntityFrame,
        ));
    }
    registry.declare(QueryDeclaration::entity(
        "sq",
        ValueType::String,
        Arguments::None,
        Rate::EntityTick,
    ));
    registry
}
