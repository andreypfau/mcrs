use crate::Host;
use anyhow::Context;
use std::time::Duration;
use tokio::time;

pub(crate) async fn advance<H: Host>(host: &mut H, ticks: u32) -> anyhow::Result<()> {
    let mut commands = vec!["tick query".to_owned(), "time query gametime".to_owned()];
    if ticks > 0 {
        commands.push(format!("tick step {ticks}"));
    }
    let status = host.console(&commands)?;
    anyhow::ensure!(
        status.iter().any(|line| line == "The game is frozen"),
        "expected a frozen server, got {status:?}"
    );
    let start = game_time(&status)?;
    let target = start + i64::from(ticks);
    let deadline = time::Instant::now() + Duration::from_secs(5);
    loop {
        let current = game_time(&host.console(&["time query gametime".to_owned()])?)?;
        if current == target {
            return Ok(());
        }
        anyhow::ensure!(
            current < target,
            "game time advanced by more than {ticks} ticks"
        );
        anyhow::ensure!(
            time::Instant::now() < deadline,
            "game time did not advance by {ticks} ticks"
        );
        time::sleep(Duration::from_millis(20)).await;
    }
}

fn game_time(output: &[String]) -> anyhow::Result<i64> {
    output
        .iter()
        .find_map(|line| {
            line.strip_prefix("The game time is ")?
                .strip_suffix(" tick(s)")
        })
        .and_then(|value| value.parse::<i64>().ok())
        .with_context(|| format!("could not read game time: {output:?}"))
}
