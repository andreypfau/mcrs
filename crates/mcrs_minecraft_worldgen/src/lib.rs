#![allow(
    clippy::collapsible_if,
    clippy::type_complexity,
    clippy::option_map_unit_fn,
    clippy::needless_late_init,
    clippy::map_flatten,
    clippy::too_many_arguments,
    clippy::unnecessary_lazy_evaluations,
    clippy::uninlined_format_args,
    clippy::unnecessary_unwrap,
    clippy::single_match,
    clippy::needless_return,
    clippy::redundant_pattern_matching,
    clippy::useless_conversion,
    clippy::unnecessary_cast,
    clippy::drain_collect,
    clippy::single_char_add_str,
    clippy::excessive_precision,
    clippy::needless_range_loop,
    clippy::clone_on_copy,
    clippy::needless_borrow,
    clippy::let_and_return,
    clippy::collapsible_match,
    dead_code,
    unused_variables,
    unused_imports,
    unused_mut,
    unused_parens,
    unreachable_pub,
    unexpected_cfgs
)]

mod climate;
pub mod density_function;
mod noise;
pub mod proto;
mod spline;

#[cfg(feature = "bevy")]
pub mod bevy;
