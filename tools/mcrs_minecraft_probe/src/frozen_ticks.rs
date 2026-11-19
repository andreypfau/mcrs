use crate::Host;
use anyhow::Context;
use std::time::Duration;
use tokio::time;

pub(crate) async fn advance<H: Host>(host: &mut H, ticks: u32) -> anyhow::Result<()> {
    let mut commands = vec!["tick query".to_owned(), "time query gametime".to_owned()];
    if ticks > 0 {
        commands.push(format!("tick step {ticks}"));
    }
    let deadline = time::Instant::now() + Duration::from_secs(5);
    let status = console_before(host, &commands, deadline)?;
    anyhow::ensure!(
        status.iter().any(|line| line == "The game is frozen"),
        "expected a frozen server, got {status:?}"
    );
    let start = game_time(&status)?;
    let target = start + i64::from(ticks);
    let query = ["time query gametime".to_owned()];
    loop {
        let current = game_time(&console_before(host, &query, deadline)?)?;
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
        time::sleep_until(deadline.min(time::Instant::now() + Duration::from_millis(20))).await;
    }
}

pub(crate) fn console_before<H: Host>(
    host: &mut H,
    lines: &[String],
    deadline: time::Instant,
) -> anyhow::Result<Vec<String>> {
    anyhow::ensure!(
        time::Instant::now() < deadline,
        "console checkpoint timed out"
    );
    let output = host.console_before(lines, deadline.into_std())?;
    anyhow::ensure!(
        time::Instant::now() < deadline,
        "console checkpoint timed out"
    );
    Ok(output)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PlayerBan;
    use mcrs_minecraft_nbt::compound::NbtCompound;
    use std::time::Instant;

    struct LateConsole;

    impl Host for LateConsole {
        fn read_player_bans(&mut self) -> anyhow::Result<Vec<PlayerBan>> {
            anyhow::bail!("unexpected ban file read")
        }

        fn read_player_file(&mut self, _player: &str) -> anyhow::Result<NbtCompound> {
            anyhow::bail!("unexpected player file read")
        }

        fn console(&mut self, _lines: &[String]) -> anyhow::Result<Vec<String>> {
            anyhow::bail!("unexpected console query without a deadline")
        }

        fn console_before(
            &mut self,
            _lines: &[String],
            deadline: Instant,
        ) -> anyhow::Result<Vec<String>> {
            std::thread::sleep(deadline.saturating_duration_since(Instant::now()));
            Ok(vec!["The game time is 42 tick(s)".to_owned()])
        }

        fn restart_server(&mut self) -> anyhow::Result<()> {
            anyhow::bail!("unexpected restart")
        }
    }

    #[test]
    fn console_reply_at_deadline_is_rejected_even_when_it_contains_the_target() {
        let deadline = time::Instant::now() + Duration::from_millis(10);
        let result = console_before(
            &mut LateConsole,
            &["time query gametime".to_owned()],
            deadline,
        );
        assert!(
            result.unwrap_err().to_string().contains("timed out"),
            "a late value is not evidence for a successful checkpoint"
        );
    }
}
