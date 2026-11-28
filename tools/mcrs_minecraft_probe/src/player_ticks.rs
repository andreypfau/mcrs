use crate::Host;
use crate::frozen_ticks::console_before;
use anyhow::Context;
use std::time::Duration;
use tokio::time;

pub(crate) async fn wait_at_least<H: Host>(
    host: &mut H,
    player: &str,
    ticks: u32,
) -> anyhow::Result<()> {
    anyhow::ensure!(
        !player.is_empty()
            && player
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_'),
        "invalid player name for tick checkpoint"
    );
    let query = [format!(
        "data get entity {player} active_effects[{{id:\"minecraft:luck\"}}].duration"
    )];
    let prefix = format!("{player} has the following entity data: ");
    let deadline = time::Instant::now() + Duration::from_secs(30);
    let start = read_duration(host, &query, &prefix, deadline)?;
    anyhow::ensure!(
        start > i64::from(ticks),
        "{player}'s luck effect expires before the checkpoint"
    );
    loop {
        let current = read_duration(host, &query, &prefix, deadline)?;
        anyhow::ensure!(current <= start, "{player}'s luck effect was refreshed");
        if start - current >= i64::from(ticks) {
            return Ok(());
        }
        time::sleep_until(deadline.min(time::Instant::now() + Duration::from_millis(250))).await;
    }
}

fn read_duration<H: Host>(
    host: &mut H,
    query: &[String],
    prefix: &str,
    deadline: time::Instant,
) -> anyhow::Result<i64> {
    let output = console_before(host, query, deadline)?;
    output
        .iter()
        .find_map(|line| line.strip_prefix(prefix))
        .and_then(|value| value.parse::<i64>().ok())
        .filter(|duration| *duration > 0)
        .with_context(|| format!("could not read the remaining luck effect duration: {output:?}"))
}
