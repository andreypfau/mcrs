use glam::DVec3;

use crate::scene::SAMPLE_PREFIX;

const GAME_TIME: [&str; 2] = ["The game time is ", "The time is "];
const COUNT: [&str; 2] = ["Test passed. Count: ", "Test passed, count: "];
const ENTITY_DATA: &str = " has the following entity data: ";
const SPRINT: &str = "Sprint completed with ";
const DAMAGE: [&str; 2] = ["Applied ", " damage to "];

#[must_use]
pub fn game_time(line: &str) -> Option<i64> {
    let (_, rest) = GAME_TIME
        .iter()
        .find_map(|marker| line.split_once(marker))?;
    rest.split(' ').next()?.trim().parse().ok()
}

#[must_use]
pub fn count(line: &str) -> Option<usize> {
    let (_, rest) = COUNT.iter().find_map(|marker| line.split_once(marker))?;
    rest.trim().parse().ok()
}

#[must_use]
pub fn count_or_zero(line: &str) -> Option<usize> {
    if line.trim_end().ends_with("Test failed") {
        return Some(0);
    }
    count(line)
}

#[must_use]
pub fn damage_applied(line: &str) -> bool {
    line.split_once(DAMAGE[0])
        .is_some_and(|(_, rest)| rest.contains(DAMAGE[1]))
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SamplePosition {
    pub index: usize,
    pub position: DVec3,
}

#[must_use]
pub fn sample_position(line: &str) -> Option<SamplePosition> {
    let (head, data) = line.split_once(ENTITY_DATA)?;
    let (_, index) = head.rsplit_once(SAMPLE_PREFIX)?;
    let index: usize = index.parse().ok()?;
    let inner = data.trim().strip_prefix('[')?.strip_suffix(']')?;
    let mut axes = inner
        .split(',')
        .map(|axis| axis.trim().trim_end_matches('d').parse::<f64>().ok());
    let (Some(Some(x)), Some(Some(y)), Some(Some(z))) = (axes.next(), axes.next(), axes.next())
    else {
        return None;
    };
    Some(SamplePosition {
        index,
        position: DVec3::new(x, y, z),
    })
}

#[must_use]
pub fn sprint_millis(line: &str) -> Option<f64> {
    let (_, rest) = line.split_once(SPRINT)?;
    let (_, rest) = rest.split_once(", or ")?;
    let (number, _) = rest.split_once(" ms")?;
    number.trim().parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_game_time_and_the_count_read_in_both_wordings() {
        let at = "[00:23:32] [Server thread/INFO]: System chat: ";
        assert_eq!(
            game_time(&format!("{at}The game time is 4 tick(s)")),
            Some(4)
        );
        assert_eq!(game_time(&format!("{at}The time is 4")), Some(4));
        assert_eq!(count(&format!("{at}Test passed. Count: 2")), Some(2));
        assert_eq!(count_or_zero(&format!("{at}Test failed")), Some(0));
    }

    #[test]
    fn a_sample_answer_names_its_pig() {
        let line = "[12:00:00] [Server thread/INFO]: mbs17 has the following entity data: [12.5d, -60.0d, -3.25d]";
        assert_eq!(
            sample_position(line),
            Some(SamplePosition {
                index: 17,
                position: DVec3::new(12.5, -60.0, -3.25),
            })
        );
    }
}
