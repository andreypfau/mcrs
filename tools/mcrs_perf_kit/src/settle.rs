use crate::stats_line::StatsLine;

/// Watches stats lines until the terrain stops changing: `lines` in a row with no mesh queued,
/// in flight or waiting for upload, and the same section count.
pub struct Settle {
    lines: usize,
    streak: usize,
    sections: Option<f64>,
}

impl Settle {
    #[must_use]
    pub fn new(lines: usize) -> Self {
        Self {
            lines,
            streak: 0,
            sections: None,
        }
    }

    /// Takes the next line and says whether the terrain has settled with it.
    pub fn observe(&mut self, line: &StatsLine) -> bool {
        let idle = [line.mesh_queued, line.mesh_in_flight, line.uploads_waiting]
            .iter()
            .all(|count| *count == Some(0.0));
        let same = line.sections.is_some() && line.sections == self.sections;
        self.streak = match (idle, same) {
            (false, _) => 0,
            (true, true) => self.streak.saturating_add(1),
            (true, false) => 1,
        };
        self.sections = line.sections;
        self.streak >= self.lines
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(sections: f64, queued: f64) -> StatsLine {
        StatsLine {
            sections: Some(sections),
            mesh_queued: Some(queued),
            mesh_in_flight: Some(0.0),
            uploads_waiting: Some(0.0),
            ..StatsLine::default()
        }
    }

    #[test]
    fn settles_after_five_idle_lines_with_the_same_section_count() {
        let lines = [
            line(100.0, 0.0),
            line(100.0, 0.0),
            line(100.0, 3.0),
            line(120.0, 0.0),
            line(120.0, 0.0),
            line(120.0, 0.0),
            line(120.0, 0.0),
            line(130.0, 0.0),
            line(130.0, 0.0),
            line(130.0, 0.0),
            line(130.0, 0.0),
            line(130.0, 0.0),
        ];
        let mut settle = Settle::new(5);
        let settled: Vec<bool> = lines.iter().map(|line| settle.observe(line)).collect();
        let first = settled.iter().position(|settled| *settled);
        assert_eq!(
            first,
            Some(11),
            "a queued mesh and a new section count restart the count"
        );
    }
}
