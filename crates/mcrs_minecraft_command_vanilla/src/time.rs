use crate::set::{VanillaCommands, VanillaContext};
use bevy_ecs::prelude::*;
use mcrs_command::{ArgumentHandle, BuildError, FloatArgument, NodeId};
use mcrs_minecraft_command::{CommandLevel, PermissionCheck};
use mcrs_minecraft_command_argument::ResourceArgument;
use mcrs_minecraft_command_argument::{TimeArgument, TimeMarkerArgument, TimelineArgument};
use mcrs_minecraft_command_engine::{
    Call, CommandFailure, CommandOutput, CommandRegistrar, CommandResult, EffectQueue, Feedback,
    Worlds,
};
use mcrs_minecraft_command_registry::DimensionClock;
use mcrs_minecraft_engine::clock::{EditClock, GameTime};
use mcrs_minecraft_environment::keys::WORLD_CLOCK;
use mcrs_minecraft_environment::world_clock::{
    self, ClockEdit, ClockState, ClockTimeMarker, WorldClock, WorldClocks,
};
use mcrs_minecraft_resource::Id;
use mcrs_minecraft_resource::ResourceLocation;
use mcrs_minecraft_text::{Text, TranslateArg};

const MIN_RATE: f32 = 1.0e-5;
const MAX_RATE: f32 = 1000.0;
const WRAP: i64 = i32::MAX as i64;

/// The clock subtree that a node belongs to: the one under `time`, or the one under
/// `of <clock>`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Clock {
    Default,
    Of,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Action {
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
pub(crate) enum Branch {
    Clock(Clock, Action),
    GameTime,
}

#[derive(Clone, Copy)]
struct Subtree {
    set: ArgumentHandle<TimeArgument>,
    marker: ArgumentHandle<TimeMarkerArgument>,
    add: ArgumentHandle<TimeArgument>,
    rate: ArgumentHandle<FloatArgument>,
    timeline: ArgumentHandle<TimelineArgument>,
}

#[derive(Clone, Copy)]
pub(crate) struct TimeCommand {
    clock: ArgumentHandle<ResourceArgument>,
    default: Subtree,
    of: Subtree,
}

fn subtree(
    registrar: &mut CommandRegistrar<'_, VanillaCommands, Branch>,
    parent: NodeId,
    which: Clock,
    clock: Option<NodeId>,
) -> Result<Subtree, BuildError> {
    let runs = |action| Branch::Clock(which, action);
    let set = registrar.literal(parent, "set").id()?;
    let set_time = registrar
        .arg(set, "time", TimeArgument { min: 0 })
        .runs(runs(Action::Set))
        .id()?;
    let marker = registrar
        .arg(set, "timemarker", TimeMarkerArgument { clock })
        .runs(runs(Action::SetMarker))
        .id()?;
    let add = registrar.literal(parent, "add").id()?;
    let add = registrar
        .arg(add, "time", TimeArgument { min: i32::MIN })
        .runs(runs(Action::Add))
        .id()?;
    let _ = registrar
        .literal(parent, "pause")
        .runs(runs(Action::Pause))
        .id()?;
    let _ = registrar
        .literal(parent, "resume")
        .runs(runs(Action::Resume))
        .id()?;
    let rate = registrar.literal(parent, "rate").id()?;
    let rate = registrar
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
    let query = registrar.literal(parent, "query").id()?;
    let _ = registrar
        .literal(query, "time")
        .runs(runs(Action::QueryTime))
        .id()?;
    let timeline = registrar
        .arg(query, "timeline", TimelineArgument { clock })
        .runs(runs(Action::QueryTimeline))
        .id()?;
    let _ = registrar
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

pub(crate) fn build(
    registrar: &mut CommandRegistrar<'_, VanillaCommands, Branch>,
) -> Result<TimeCommand, BuildError> {
    let time = registrar
        .literal(NodeId::ROOT, "time")
        .requires(PermissionCheck::Require(CommandLevel::Gamemasters))
        .id()?;
    let default = subtree(registrar, time, Clock::Default, None)?;
    let of = registrar.literal(time, "of").id()?;
    let clock = registrar
        .arg(
            of,
            "clock",
            ResourceArgument {
                registry: WORLD_CLOCK.location(),
            },
        )
        .id()?;
    let of = subtree(registrar, clock.node(), Clock::Of, Some(clock.node()))?;
    let query = registrar.literal(time, "query").id()?;
    let _ = registrar
        .literal(query, "gametime")
        .runs(Branch::GameTime)
        .id()?;
    Ok(TimeCommand { clock, default, of })
}

fn wrapped(value: i64) -> i32 {
    i32::try_from(value % WRAP).unwrap_or_default()
}

type MessageArg = TranslateArg<()>;

/// What a branch does to the clock: the change it asks for, its message and its result.
struct Outcome {
    edit: Option<ClockEdit>,
    message: Text,
    result: i32,
}

impl Outcome {
    fn change(edit: ClockEdit, message: Text, result: i32) -> Self {
        Self {
            edit: Some(edit),
            message,
            result,
        }
    }

    fn query(message: Text, result: i32) -> Self {
        Self {
            edit: None,
            message,
            result,
        }
    }
}

/// The time marker of the clock with that name, from every timeline that runs on the clock.
fn time_marker(
    context: &VanillaContext,
    clock: Id<WorldClock>,
    name: ResourceLocation<'_>,
) -> Option<ClockTimeMarker> {
    context
        .lookup()
        .registry_lookup()
        .timelines(Some(clock))
        .find_map(|(_, timeline)| {
            let (_, marker) = timeline
                .time_markers
                .iter()
                .find(|(id, _)| name == id.as_str())?;
            Some(ClockTimeMarker {
                ticks: marker.ticks,
                period_ticks: timeline.period_ticks,
                show_in_commands: marker.show_in_commands,
            })
        })
}

/// The clock a branch acts on, and what the branch reads its arguments with.
struct ClockCall<'c, 'w, 's> {
    call: &'c Call<Branch, TimeCommand>,
    context: &'c VanillaContext<'w, 's>,
    subtree: &'c Subtree,
    clock: Id<WorldClock>,
}

impl ClockCall<'_, '_, '_> {
    fn name(&self) -> MessageArg {
        self.context
            .lookup()
            .registry_lookup()
            .clock_name(self.clock)
            .map(ToString::to_string)
            .unwrap_or_default()
            .into()
    }

    fn act(&self, action: Action, state: ClockState) -> Result<Outcome, CommandFailure> {
        match action {
            Action::Set => self.set(state.total_ticks),
            Action::SetMarker => self.set_marker(state.total_ticks),
            Action::Add => self.add(state),
            Action::Pause | Action::Resume => self.pause_or_resume(action, state),
            Action::Rate => self.rate(state),
            Action::QueryTime => Ok(Outcome::query(
                Text::translate_args(
                    "commands.time.query.absolute",
                    vec![self.name(), state.total_ticks.into()],
                ),
                wrapped(state.total_ticks),
            )),
            Action::QueryTimeline | Action::QueryRepetition => {
                self.query_timeline(action, state.total_ticks)
            }
        }
    }

    fn set(&self, total: i64) -> Result<Outcome, CommandFailure> {
        let ticks = self.context.arg(&self.call.invocation, self.subtree.set)?;
        if total == i64::from(ticks) {
            return Err(CommandFailure::translate(
                "commands.time.set.already_at_time",
                vec![self.name(), ticks.into()],
            ));
        }
        Ok(Outcome::change(
            ClockEdit::SetTotalTicks(i64::from(ticks)),
            Text::translate_args(
                "commands.time.set.absolute",
                vec![self.name(), ticks.into()],
            ),
            ticks,
        ))
    }

    fn add(&self, state: ClockState) -> Result<Outcome, CommandFailure> {
        let ticks = self.context.arg(&self.call.invocation, self.subtree.add)?;
        let mut next = state;
        next.apply(ClockEdit::AddTicks(ticks));
        Ok(Outcome::change(
            ClockEdit::AddTicks(ticks),
            Text::translate_args(
                "commands.time.set.absolute",
                vec![self.name(), next.total_ticks.into()],
            ),
            wrapped(next.total_ticks),
        ))
    }

    fn pause_or_resume(
        &self,
        action: Action,
        state: ClockState,
    ) -> Result<Outcome, CommandFailure> {
        let pause = action == Action::Pause;
        if state.paused == pause {
            let key = if pause {
                "commands.time.pause.already_paused"
            } else {
                "commands.time.pause.already_running"
            };
            return Err(CommandFailure::translate(key, vec![self.name()]));
        }
        let key = if pause {
            "commands.time.pause"
        } else {
            "commands.time.resume"
        };
        Ok(Outcome::change(
            ClockEdit::SetPaused(pause),
            Text::translate_args(key, vec![self.name()]),
            1,
        ))
    }

    fn rate(&self, state: ClockState) -> Result<Outcome, CommandFailure> {
        let rate = self.context.arg(&self.call.invocation, self.subtree.rate)?;
        if state.rate == rate {
            return Err(CommandFailure::translate(
                "commands.time.rate.already_same",
                vec![self.name(), rate.into()],
            ));
        }
        Ok(Outcome::change(
            ClockEdit::SetRate(rate),
            Text::translate_args("commands.time.rate", vec![self.name(), rate.into()]),
            1,
        ))
    }

    fn set_marker(&self, total: i64) -> Result<Outcome, CommandFailure> {
        let id = self
            .context
            .arg(&self.call.invocation, self.subtree.marker)?;
        let Some(marker) = time_marker(self.context, self.clock, id) else {
            return Err(CommandFailure::translate(
                "commands.time.no_time_marker_found",
                vec![id.to_string().into(), self.name()],
            ));
        };
        if marker.occurs_at(total) {
            return Err(CommandFailure::translate(
                "commands.time.set.already_at_time_marker",
                vec![self.name(), id.to_string().into()],
            ));
        }
        let next = marker.next_after(total);
        Ok(Outcome::change(
            ClockEdit::SetTotalTicks(next),
            Text::translate_args(
                "commands.time.set.time_marker",
                vec![self.name(), id.to_string().into()],
            ),
            wrapped(next),
        ))
    }

    fn query_timeline(&self, action: Action, total: i64) -> Result<Outcome, CommandFailure> {
        let id = self
            .context
            .arg(&self.call.invocation, self.subtree.timeline)?;
        let Some((timeline_name, timeline)) = self.context.lookup().registry_lookup().timeline(id)
        else {
            return Err(CommandFailure::failed());
        };
        let timeline_name: MessageArg = timeline_name.to_string().into();
        if timeline.clock != self.clock {
            return Err(CommandFailure::translate(
                "commands.time.wrong_timeline_for_clock",
                vec![timeline_name, self.name()],
            ));
        }
        let period = timeline.period_ticks.map(i64::from);
        let (key, value) = if action == Action::QueryTimeline {
            let value = period.and_then(|p| total.checked_rem(p)).unwrap_or(total);
            ("commands.time.query.timeline", value)
        } else {
            // The 64-bit quotient, where the game truncates it to 32 bits.
            let value = period.and_then(|p| total.checked_div(p)).unwrap_or(0);
            ("commands.time.query.timeline.repetitions", value)
        };
        Ok(Outcome::query(
            Text::translate_args(key, vec![timeline_name, value.into()]),
            wrapped(value),
        ))
    }
}

/// The clock that the branch acts on.
fn selected_clock(
    call: &Call<Branch, TimeCommand>,
    command: &TimeCommand,
    context: &VanillaContext,
    which: Clock,
) -> Result<Id<WorldClock>, CommandFailure> {
    match which {
        Clock::Of => {
            let name = context.arg(&call.invocation, command.clock)?;
            context
                .lookup()
                .registry_lookup()
                .clock(Some(name))
                .ok_or_else(CommandFailure::failed)
        }
        Clock::Default => {
            let Some(DimensionClock {
                dimension_type,
                clock,
            }) = context.lookup().registry_lookup().dimension_clock()
            else {
                return Err(CommandFailure::failed());
            };
            clock.ok_or_else(|| {
                CommandFailure::translate(
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

pub(crate) fn run(
    In(call): In<Call<Branch, TimeCommand>>,
    context: VanillaContext,
    clocks: Res<WorldClocks>,
    game_time: Res<GameTime>,
    mut feedback: CommandOutput,
    mut edits: ResMut<EffectQueue<EditClock>>,
) -> CommandResult {
    let command = call.command;
    let (which, action) = match call.branch {
        Branch::GameTime => {
            let time = game_time.0;
            feedback.success(
                &call.invocation,
                Text::translate_args("commands.time.query.gametime", vec![time.into()]),
                Feedback::Source,
            );
            return Ok(wrapped(time));
        }
        Branch::Clock(which, action) => (which, action),
    };
    let clock = selected_clock(&call, &command, &context, which)?;
    let state = *clocks.get(clock).ok_or_else(CommandFailure::failed)?;
    let subtree = match which {
        Clock::Default => &command.default,
        Clock::Of => &command.of,
    };
    let clock_call = ClockCall {
        call: &call,
        context: &context,
        subtree,
        clock,
    };
    let outcome = clock_call.act(action, state)?;
    let audience = match outcome.edit {
        Some(edit) => {
            feedback.effect(&mut edits, EditClock { clock, edit });
            Feedback::SourceAndOperators
        }
        None => Feedback::Source,
    };
    feedback.success(&call.invocation, outcome.message, audience);
    Ok(outcome.result)
}

/// Applies a clock edit on the host, which owns the clocks, then extracts the clocks into every
/// world, so the rest of the run reads the new times.
pub(crate) fn apply(worlds: &mut Worlds, edit: EditClock) {
    let (host, dimensions) = worlds.host_and_worlds();
    host.trigger(edit);
    for world in dimensions {
        world_clock::extract_world_clocks(host, world);
    }
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
        ] {
            assert_eq!(wrapped(value), result, "{value}");
        }
    }
}
