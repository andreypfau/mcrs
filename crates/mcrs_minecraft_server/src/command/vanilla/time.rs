use crate::command::{
    Call, CommandContext, CommandFailure, CommandFeedback, CommandRegistrar, CommandResult,
};
use crate::world::clock::{GameTime, PendingClockEdits};
use bevy_ecs::prelude::*;
use mcrs_command::{Arg, BuildError, FloatArgument, NodeId};
use mcrs_minecraft_command::{
    CommandLevel, PermissionCheck, ResourceArgument, TimeArgument, TimeMarkerArgument,
    TimelineArgument,
};
use mcrs_minecraft_core::Id;
use mcrs_minecraft_engine::bus::EditClock;
use mcrs_minecraft_engine::channel_types::FromDim;
use mcrs_minecraft_environment::keys::{TIMELINE, WORLD_CLOCK};
use mcrs_minecraft_environment::world_clock::{
    ClockEdit, ClockState, ClockTimeMarker, WorldClock, WorldClocks,
};
use mcrs_minecraft_level::world::channels::FromDimSender;
use mcrs_minecraft_protocol::text::{Text, TranslateArg};

const MIN_RATE: f32 = 1.0e-5;
const MAX_RATE: f32 = 1000.0;
const WRAP: i64 = i32::MAX as i64;

/// The clock subtree that a node belongs to: the one under `time`, or the one under
/// `of <clock>`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Clock {
    Default,
    Of,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Action {
    Set,
    SetMarker,
    Add,
    Pause,
    Resume,
    Rate,
    QueryTime,
    QueryTimeline,
    QueryRepetition,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Branch {
    Clock(Clock, Action),
    GameTime,
}

#[derive(Clone, Copy)]
struct Subtree {
    set: Arg<TimeArgument>,
    marker: Arg<TimeMarkerArgument>,
    add: Arg<TimeArgument>,
    rate: Arg<FloatArgument>,
    timeline: Arg<TimelineArgument>,
}

#[derive(Clone, Copy)]
pub(super) struct TimeCommand {
    clock: Arg<ResourceArgument>,
    default: Subtree,
    of: Subtree,
}

fn subtree(
    r: &mut CommandRegistrar<'_, Branch>,
    parent: NodeId,
    which: Clock,
    clock: Option<NodeId>,
) -> Result<Subtree, BuildError> {
    let runs = |action| Branch::Clock(which, action);
    let set = r.literal(parent, "set").id()?;
    let set_time = r
        .arg(set, "time", TimeArgument { min: 0 })
        .runs(runs(Action::Set))
        .id()?;
    let marker = r
        .arg(set, "timemarker", TimeMarkerArgument { clock })
        .runs(runs(Action::SetMarker))
        .id()?;
    let add = r.literal(parent, "add").id()?;
    let add = r
        .arg(add, "time", TimeArgument { min: i32::MIN })
        .runs(runs(Action::Add))
        .id()?;
    let _ = r.literal(parent, "pause").runs(runs(Action::Pause)).id()?;
    let _ = r
        .literal(parent, "resume")
        .runs(runs(Action::Resume))
        .id()?;
    let rate = r.literal(parent, "rate").id()?;
    let rate = r
        .arg(
            rate,
            "rate",
            FloatArgument {
                min: MIN_RATE,
                max: MAX_RATE,
            },
        )
        .runs(runs(Action::Rate))
        .id()?;
    let query = r.literal(parent, "query").id()?;
    let _ = r
        .literal(query, "time")
        .runs(runs(Action::QueryTime))
        .id()?;
    let timeline = r
        .arg(
            query,
            "timeline",
            TimelineArgument {
                resource: ResourceArgument {
                    registry: TIMELINE.location(),
                },
                clock,
            },
        )
        .runs(runs(Action::QueryTimeline))
        .id()?;
    let _ = r
        .literal(timeline, "repetition")
        .runs(runs(Action::QueryRepetition))
        .id()?;
    Ok(Subtree {
        set: set_time,
        marker,
        add,
        rate,
        timeline,
    })
}

/// CMD-TIME-001.
pub(super) fn build(r: &mut CommandRegistrar<'_, Branch>) -> Result<TimeCommand, BuildError> {
    let time = r
        .literal(NodeId::ROOT, "time")
        .requires(PermissionCheck::Require(CommandLevel::Gamemasters))
        .id()?;
    let default = subtree(r, time, Clock::Default, None)?;
    let of = r.literal(time, "of").id()?;
    let clock = r
        .arg(
            of,
            "clock",
            ResourceArgument {
                registry: WORLD_CLOCK.location(),
            },
        )
        .id()?;
    let of = subtree(r, clock.node(), Clock::Of, Some(clock.node()))?;
    let query = r.literal(time, "query").id()?;
    let _ = r.literal(query, "gametime").runs(Branch::GameTime).id()?;
    Ok(TimeCommand { clock, default, of })
}

/// CMD-TIME-017.
fn wrapped(value: i64) -> i32 {
    i32::try_from(value % WRAP).unwrap_or_default()
}

type MessageArg = TranslateArg<mcrs_minecraft_item::Template<mcrs_minecraft_registries::Named>>;

fn message(key: &'static str, with: Vec<MessageArg>) -> Text {
    Text::translate_args(key, with)
}

fn failure(key: &'static str, with: Vec<MessageArg>) -> CommandFailure {
    CommandFailure(message(key, with))
}

/// What a branch does to the clock: the change it asks for, its message and its result.
struct Outcome {
    edit: Option<ClockEdit>,
    message: Text,
    broadcast: bool,
    result: i32,
}

impl Outcome {
    fn change(edit: ClockEdit, message: Text, result: i32) -> Self {
        Self {
            edit: Some(edit),
            message,
            broadcast: true,
            result,
        }
    }

    fn query(message: Text, result: i32) -> Self {
        Self {
            edit: None,
            message,
            broadcast: false,
            result,
        }
    }
}

/// The time marker of the clock with that name, from every timeline that runs on the clock
/// (CMD-TIME-007).
fn time_marker(
    context: &CommandContext,
    clock: Id<WorldClock>,
    name: mcrs_minecraft_command::Identifier<'_>,
) -> Option<ClockTimeMarker> {
    context.timelines(Some(clock)).find_map(|(_, timeline)| {
        let (_, marker) = timeline
            .time_markers
            .iter()
            .find(|(id, _)| name.eq_str(id.as_str()))?;
        Some(ClockTimeMarker {
            ticks: marker.ticks,
            period_ticks: timeline.period_ticks,
            show_in_commands: marker.show_in_commands,
        })
    })
}

fn act(
    call: &Call<Branch, TimeCommand>,
    context: &CommandContext,
    action: Action,
    subtree: &Subtree,
    clock: Id<WorldClock>,
    state: ClockState,
) -> Result<Outcome, CommandFailure> {
    let name = || -> MessageArg {
        context
            .clock_name(clock)
            .map(ToString::to_string)
            .unwrap_or_default()
            .into()
    };
    let total = state.total_ticks;
    Ok(match action {
        Action::Set => {
            let ticks = context.arg(call, subtree.set)?;
            if total == i64::from(ticks) {
                return Err(failure(
                    "commands.time.set.already_at_time",
                    vec![name(), ticks.into()],
                ));
            }
            Outcome::change(
                ClockEdit::SetTotalTicks(i64::from(ticks)),
                message("commands.time.set.absolute", vec![name(), ticks.into()]),
                ticks,
            )
        }
        Action::SetMarker => set_marker(call, context, subtree, clock, total, name())?,
        Action::Add => {
            let ticks = context.arg(call, subtree.add)?;
            let mut next = state;
            next.apply(ClockEdit::AddTicks(ticks));
            Outcome::change(
                ClockEdit::AddTicks(ticks),
                message(
                    "commands.time.set.absolute",
                    vec![name(), next.total_ticks.into()],
                ),
                wrapped(next.total_ticks),
            )
        }
        Action::Pause | Action::Resume => {
            let pause = action == Action::Pause;
            if state.paused == pause {
                let key = if pause {
                    "commands.time.pause.already_paused"
                } else {
                    "commands.time.pause.already_running"
                };
                return Err(failure(key, vec![name()]));
            }
            let key = if pause {
                "commands.time.pause"
            } else {
                "commands.time.resume"
            };
            Outcome::change(ClockEdit::SetPaused(pause), message(key, vec![name()]), 1)
        }
        Action::Rate => {
            let rate = context.arg(call, subtree.rate)?;
            if state.rate == rate {
                return Err(failure(
                    "commands.time.rate.already_same",
                    vec![name(), rate.into()],
                ));
            }
            Outcome::change(
                ClockEdit::SetRate(rate),
                message("commands.time.rate", vec![name(), rate.into()]),
                1,
            )
        }
        Action::QueryTime => Outcome::query(
            message("commands.time.query.absolute", vec![name(), total.into()]),
            wrapped(total),
        ),
        Action::QueryTimeline | Action::QueryRepetition => {
            query_timeline(call, context, action, subtree, clock, total, name())?
        }
    })
}

/// CMD-TIME-011.
fn set_marker(
    call: &Call<Branch, TimeCommand>,
    context: &CommandContext,
    subtree: &Subtree,
    clock: Id<WorldClock>,
    total: i64,
    name: MessageArg,
) -> Result<Outcome, CommandFailure> {
    let id = context.arg(call, subtree.marker)?;
    let Some(marker) = time_marker(context, clock, id) else {
        return Err(failure(
            "commands.time.no_time_marker_found",
            vec![id.to_string().into(), name],
        ));
    };
    if marker.occurs_at(total) {
        return Err(failure(
            "commands.time.set.already_at_time_marker",
            vec![name, id.to_string().into()],
        ));
    }
    let next = marker.next_after(total);
    Ok(Outcome::change(
        ClockEdit::SetTotalTicks(next),
        message(
            "commands.time.set.time_marker",
            vec![name, id.to_string().into()],
        ),
        wrapped(next),
    ))
}

/// CMD-TIME-016.
fn query_timeline(
    call: &Call<Branch, TimeCommand>,
    context: &CommandContext,
    action: Action,
    subtree: &Subtree,
    clock: Id<WorldClock>,
    total: i64,
    name: MessageArg,
) -> Result<Outcome, CommandFailure> {
    let id = context.arg(call, subtree.timeline)?;
    let Some((timeline_name, timeline)) = context.timeline(id) else {
        return Err(CommandFailure::failed());
    };
    let timeline_name: MessageArg = timeline_name.to_string().into();
    if timeline.clock != clock {
        return Err(failure(
            "commands.time.wrong_timeline_for_clock",
            vec![timeline_name, name],
        ));
    }
    let period = timeline.period_ticks.map(i64::from);
    let (key, value) = if action == Action::QueryTimeline {
        let value = period.and_then(|p| total.checked_rem(p)).unwrap_or(total);
        ("commands.time.query.timeline", value)
    } else {
        // CMD-TIME-021: the 64-bit quotient, where the game truncates it to 32 bits.
        let value = period.and_then(|p| total.checked_div(p)).unwrap_or(0);
        ("commands.time.query.timeline.repetitions", value)
    };
    Ok(Outcome::query(
        message(key, vec![timeline_name, value.into()]),
        wrapped(value),
    ))
}

/// The clock that the branch acts on (CMD-TIME-008).
fn selected_clock(
    call: &Call<Branch, TimeCommand>,
    command: &TimeCommand,
    context: &CommandContext,
    which: Clock,
) -> Result<Id<WorldClock>, CommandFailure> {
    match which {
        Clock::Of => {
            let name = context.arg(call, command.clock)?;
            context
                .clock_id(Some(name))
                .ok_or_else(CommandFailure::failed)
        }
        Clock::Default => {
            let Some((dimension_type, clock)) = context.dimension_clock() else {
                return Err(CommandFailure::failed());
            };
            clock.ok_or_else(|| {
                failure(
                    "commands.time.no_default_clock",
                    vec![
                        dimension_type
                            .map(ToString::to_string)
                            .unwrap_or_default()
                            .into(),
                    ],
                )
            })
        }
    }
}

pub(super) fn run(
    In(call): In<Call<Branch, TimeCommand>>,
    context: CommandContext,
    clocks: Res<WorldClocks>,
    game_time: Res<GameTime>,
    mut pending: ResMut<PendingClockEdits>,
    host: Res<FromDimSender<FromDim>>,
    mut feedback: CommandFeedback,
) -> CommandResult {
    let command = call.command;
    let (which, action) = match call.branch {
        Branch::GameTime => {
            let time = game_time.0;
            feedback.success(
                &call,
                message("commands.time.query.gametime", vec![time.into()]),
                false,
            );
            return Ok(wrapped(time));
        }
        Branch::Clock(which, action) => (which, action),
    };
    let clock = selected_clock(&call, &command, &context, which)?;
    let state = pending
        .state(&clocks, clock)
        .ok_or_else(CommandFailure::failed)?;
    let subtree = match which {
        Clock::Default => &command.default,
        Clock::Of => &command.of,
    };
    let outcome = act(&call, &context, action, subtree, clock, state)?;
    if let Some(edit) = outcome.edit {
        let edit = EditClock { clock, edit };
        host.0
            .try_send(FromDim::EditClock(edit))
            .map_err(|_| CommandFailure::failed())?;
        pending.push(edit);
    }
    feedback.success(&call, outcome.message, outcome.broadcast);
    Ok(outcome.result)
}

#[cfg(test)]
mod tests {
    use super::wrapped;

    #[test]
    fn a_result_wraps_at_two_to_the_31_minus_one() {
        for (value, result) in [
            (2_147_483_646, 2_147_483_646),
            (2_147_483_647, 0),
            (2_147_483_648, 1),
            (50_000, 50_000),
        ] {
            assert_eq!(wrapped(value), result, "{value}");
        }
    }
}
