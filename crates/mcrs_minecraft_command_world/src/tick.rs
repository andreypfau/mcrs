use bevy_ecs::prelude::*;
use mcrs_command::{ArgumentHandle, BuildError, FloatArgument, NodeId};
use mcrs_minecraft_command::{CommandLevel, PermissionCheck};
use mcrs_minecraft_command_argument::TimeArgument;
use mcrs_minecraft_command_engine::{
    Call, CommandFailure, CommandOutput, CommandRegistrar, CommandResult, EffectQueue, Feedback,
};
use mcrs_minecraft_command_vanilla::{VanillaCommands, VanillaContext};
use mcrs_minecraft_engine::server_edit::ServerEdit;
use mcrs_minecraft_engine_world::tick_pace::{SprintReport, TICK_SAMPLES, TickPace};
use mcrs_minecraft_text::{IntoText, Text};

use crate::server_change::ServerChange;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Branch {
    Query,
    Rate,
    Freeze,
    Unfreeze,
    Step,
    StepOne,
    StepStop,
    Sprint,
    SprintStop,
}

#[derive(Clone, Copy)]
pub(crate) struct TickCommand {
    rate: ArgumentHandle<FloatArgument>,
    step: ArgumentHandle<TimeArgument>,
    sprint: ArgumentHandle<TimeArgument>,
}

pub(crate) fn build(
    registrar: &mut CommandRegistrar<'_, VanillaCommands, Branch>,
) -> Result<TickCommand, BuildError> {
    let tick = registrar
        .literal(NodeId::ROOT, "tick")
        .requires(PermissionCheck::Require(CommandLevel::Admins))
        .id()?;
    let _ = registrar
        .literal(tick, "query")
        .runs(Branch::Query)
        .in_any_world()
        .id()?;
    let rate = registrar.literal(tick, "rate").id()?;
    let rate = registrar
        .arg(
            rate,
            "rate",
            FloatArgument {
                min: 1.0,
                max: 10000.0,
            },
        )
        .runs(Branch::Rate)
        .in_any_world()
        .id()?;
    let step = registrar
        .literal(tick, "step")
        .runs(Branch::StepOne)
        .in_any_world()
        .id()?;
    let _ = registrar
        .literal(step, "stop")
        .runs(Branch::StepStop)
        .in_any_world()
        .id()?;
    let step = registrar
        .arg(step, "time", TimeArgument { min: 1 })
        .runs(Branch::Step)
        .in_any_world()
        .id()?;
    let sprint = registrar.literal(tick, "sprint").id()?;
    let _ = registrar
        .literal(sprint, "stop")
        .runs(Branch::SprintStop)
        .in_any_world()
        .id()?;
    let sprint = registrar
        .arg(sprint, "time", TimeArgument { min: 1 })
        .runs(Branch::Sprint)
        .in_any_world()
        .id()?;
    let _ = registrar
        .literal(tick, "unfreeze")
        .runs(Branch::Unfreeze)
        .in_any_world()
        .id()?;
    let _ = registrar
        .literal(tick, "freeze")
        .runs(Branch::Freeze)
        .in_any_world()
        .id()?;
    Ok(TickCommand { rate, step, sprint })
}

fn millis_text(nanos: u64) -> String {
    format!("{:.1}", nanos as f32 / 1.0e6)
}

fn rate_text(rate: f32) -> String {
    format!("{rate:.1}")
}

pub(crate) fn query_messages(pace: &TickPace) -> [Text; 3] {
    let average = pace.average_nanos();
    let status = if pace.is_sprinting() {
        "commands.tick.status.sprinting"
    } else if pace.is_frozen() {
        "commands.tick.status.frozen"
    } else if pace.target_nanos() < average {
        "commands.tick.status.lagging"
    } else {
        "commands.tick.status.running"
    };
    let rate = if pace.is_sprinting() {
        Text::translate(
            "commands.tick.query.rate.sprinting",
            vec![
                rate_text(pace.rate()).into_text(),
                millis_text(average).into_text(),
            ],
        )
    } else {
        Text::translate(
            "commands.tick.query.rate.running",
            vec![
                rate_text(pace.rate()).into_text(),
                millis_text(average).into_text(),
                millis_text(pace.target_nanos()).into_text(),
            ],
        )
    };
    let mut sorted = *pace.samples();
    sorted.sort_unstable();
    let entry = |index: usize| millis_text(sorted.get(index).copied().unwrap_or_default());
    let percentiles = Text::translate_args(
        "commands.tick.query.percentiles",
        vec![
            entry(50).into_text().into(),
            entry(95).into_text().into(),
            entry(99).into_text().into(),
            i32::try_from(TICK_SAMPLES).unwrap_or(i32::MAX).into(),
        ],
    );
    [Text::translate(status, vec![]), rate, percentiles]
}

pub(crate) fn report_message(report: SprintReport, pace: &TickPace) -> Text {
    let target_millis = pace.target_nanos() as f64 / 1.0e6;
    Text::translate_args(
        "commands.tick.sprint.report",
        vec![
            report.ticks_per_second().into(),
            format!("{:.2}", report.millis_per_tick(target_millis))
                .into_text()
                .into(),
        ],
    )
}

struct Change {
    edit: ServerEdit,
    message: Text,
    result: i32,
}

impl Change {
    fn new(edit: ServerEdit, key: &'static str, result: i32) -> Self {
        Self {
            edit,
            message: Text::translate(key, vec![]),
            result,
        }
    }
}

fn change(
    call: &Call<Branch, TickCommand>,
    context: &VanillaContext,
    pace: &TickPace,
) -> Result<Change, CommandFailure> {
    let fail = |key: &'static str| Err(CommandFailure::translate(key, vec![]));
    let step = |ticks: i32| Change {
        edit: ServerEdit::Step {
            ticks: u64::try_from(ticks).unwrap_or_default(),
        },
        message: Text::translate_args("commands.tick.step.success", vec![ticks.into()]),
        result: 1,
    };
    match call.branch {
        Branch::Query => Err(CommandFailure::failed()),
        Branch::Rate => {
            let rate = context.arg(&call.invocation, call.command.rate)?;
            Ok(Change {
                edit: ServerEdit::TickRate { rate },
                message: Text::translate(
                    "commands.tick.rate.success",
                    vec![rate_text(rate).into_text()],
                ),
                result: rate as i32,
            })
        }
        Branch::Freeze => Ok(Change::new(
            ServerEdit::Freeze,
            "commands.tick.status.frozen",
            1,
        )),
        Branch::Unfreeze => Ok(Change::new(
            ServerEdit::Unfreeze,
            "commands.tick.status.running",
            0,
        )),
        Branch::Step | Branch::StepOne if !pace.is_frozen() => fail("commands.tick.step.fail"),
        Branch::Step => Ok(step(context.arg(&call.invocation, call.command.step)?)),
        Branch::StepOne => Ok(step(1)),
        Branch::StepStop if pace.steps() == 0 => fail("commands.tick.step.stop.fail"),
        Branch::StepStop => Ok(Change::new(
            ServerEdit::StopStep,
            "commands.tick.step.stop.success",
            1,
        )),
        Branch::Sprint => {
            let ticks = context.arg(&call.invocation, call.command.sprint)?;
            Ok(Change::new(
                ServerEdit::StartSprint {
                    ticks: u64::try_from(ticks).unwrap_or_default(),
                },
                "commands.tick.status.sprinting",
                1,
            ))
        }
        Branch::SprintStop if !pace.is_sprinting() => fail("commands.tick.sprint.stop.fail"),
        Branch::SprintStop => Ok(Change::new(
            ServerEdit::StopSprint,
            "commands.tick.sprint.stop.success",
            1,
        )),
    }
}

pub(crate) fn run(
    In(call): In<Call<Branch, TickCommand>>,
    context: VanillaContext,
    pace: Option<Res<TickPace>>,
    mut output: CommandOutput,
    mut changes: ResMut<EffectQueue<ServerChange>>,
) -> CommandResult {
    let Some(pace) = pace else {
        return Err(CommandFailure::failed());
    };
    if call.branch == Branch::Query {
        for message in query_messages(&pace) {
            output.success(&call.invocation, message, Feedback::Source);
        }
        return Ok(pace.rate() as i32);
    }
    let change = change(&call, &context, &pace)?;
    if call.branch == Branch::Sprint && pace.is_sprinting() {
        output.success(
            &call.invocation,
            Text::translate("commands.tick.sprint.stop.success", vec![]),
            Feedback::SourceAndOperators,
        );
    }
    output.effect(&mut changes, ServerChange(change.edit));
    output.success(
        &call.invocation,
        change.message,
        Feedback::SourceAndOperators,
    );
    Ok(change.result)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(
        values: &[&str],
    ) -> Vec<mcrs_minecraft_text::TranslateArg<mcrs_minecraft_text::ItemHover>> {
        values
            .iter()
            .map(|value| value.to_string().into_text().into())
            .collect()
    }

    #[test]
    fn the_query_reports_the_state_the_rate_and_the_sorted_percentiles() {
        let mut pace = TickPace::new(20);
        for millis in (1..=100).rev() {
            pace.begin_tick();
            pace.end_tick(millis * 1_000_000);
        }
        let [status, rate, percentiles] = query_messages(&pace);
        assert_eq!(
            status,
            Text::translate("commands.tick.status.lagging", vec![])
        );
        assert_eq!(
            rate,
            Text::translate_args(
                "commands.tick.query.rate.running",
                texts(&["20.0", "50.5", "50.0"])
            )
        );
        let mut expected = texts(&["51.0", "96.0", "100.0"]);
        expected.push(100.into());
        assert_eq!(
            percentiles,
            Text::translate_args("commands.tick.query.percentiles", expected)
        );
    }

    #[test]
    fn the_report_truncates_the_ticks_per_second_and_rounds_the_milliseconds() {
        let pace = TickPace::new(20);
        let report = SprintReport {
            ticks: 200,
            nanos: 1_234_567_890,
        };
        let mut expected = vec![162.into()];
        expected.extend(texts(&["6.17"]));
        assert_eq!(
            report_message(report, &pace),
            Text::translate_args("commands.tick.sprint.report", expected)
        );
    }
}
