use std::collections::BTreeMap;

use serde::Serialize;

use crate::median::median;

const TARGET: &str = "mcrs_client::stats: ";

/// One `MCRS_STATS` line of the client; a field the line did not carry is `None`.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct StatsLine {
    pub fps: Option<f64>,
    pub frame_median_ms: Option<f64>,
    pub frame_p99_ms: Option<f64>,
    pub frame_max_ms: Option<f64>,
    pub physical_width: Option<f64>,
    pub physical_height: Option<f64>,
    pub scale: Option<f64>,
    pub engine_median_ms: Option<f64>,
    pub engine_p99_ms: Option<f64>,
    pub engine_max_ms: Option<f64>,
    pub engine_frames: Option<f64>,
    pub cpu_median_ms: Stages,
    pub cpu_p99_ms: Stages,
    pub cpu_max_ms: Stages,
    pub terrain_draws: Option<f64>,
    pub sky_draws: Option<f64>,
    pub upload_kb: Option<f64>,
    pub upload_budget_kb: Option<f64>,
    /// `GPU <slot>: X ms` by slot name; absent when the adapter has no timestamp queries.
    pub gpu_ms: BTreeMap<String, f64>,
    pub triangles: Option<f64>,
    pub triangles_hidden: Option<f64>,
    pub sections: Option<f64>,
    pub sections_total: Option<f64>,
    pub columns: Option<f64>,
    pub evicted: Option<f64>,
    pub mesh_queued: Option<f64>,
    pub mesh_in_flight: Option<f64>,
    pub uploads_waiting: Option<f64>,
    pub gpu_memory_mib: Option<f64>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Stages {
    pub main: Option<f64>,
    pub extract: Option<f64>,
    pub prepare: Option<f64>,
    pub acquire: Option<f64>,
    pub render: Option<f64>,
    pub cleanup: Option<f64>,
}

impl StatsLine {
    /// Parses a log line of the `mcrs_client::stats` target, with or without ANSI colours.
    #[must_use]
    pub fn parse(log_line: &str) -> Option<Self> {
        let plain = strip_ansi(log_line);
        let (_, body) = plain.split_once(TARGET)?;
        let mut line = Self::default();
        for entry in body.split(" | ") {
            line.read_entry(entry.trim());
        }
        Some(line)
    }

    fn read_entry(&mut self, entry: &str) {
        let values = numbers(entry);
        let at = |index: usize| values.get(index).copied();
        let fps_entry = entry
            .split_once(" fps")
            .is_some_and(|(count, _)| count.parse::<u32>().is_ok());
        if fps_entry {
            self.fps = at(0);
            self.frame_median_ms = at(1);
            self.frame_p99_ms = at(2);
            self.frame_max_ms = at(3);
        } else if entry.contains(" physical, scale ") {
            self.physical_width = at(0);
            self.physical_height = at(1);
            self.scale = at(2);
        } else if entry.starts_with("Engine:") {
            self.engine_median_ms = at(0);
            self.engine_p99_ms = at(1);
            self.engine_max_ms = at(2);
            self.engine_frames = at(3);
        } else if let Some(tails) = entry.strip_prefix("CPU p99/max: ") {
            self.read_tails(tails);
        } else if entry.starts_with("CPU: ") {
            self.cpu_median_ms = Stages {
                main: at(0),
                extract: at(1),
                prepare: at(2),
                acquire: at(3),
                render: at(4),
                cleanup: at(5),
            };
        } else if entry.starts_with("Draws: ") {
            self.terrain_draws = at(0);
            self.sky_draws = at(1);
        } else if entry.starts_with("Upload: ") {
            self.upload_kb = at(0);
            self.upload_budget_kb = at(1);
        } else if entry.starts_with("GPU memory: ") {
            self.gpu_memory_mib = at(0);
        } else if let Some(slot) = entry.strip_prefix("GPU ") {
            if let (Some((name, _)), Some(ms)) = (slot.split_once(':'), at(0)) {
                self.gpu_ms.insert(name.to_owned(), ms);
            }
        } else if entry.starts_with("Tris: ") {
            self.triangles = at(0);
            self.triangles_hidden = at(1);
        } else if entry.starts_with("Sections: ") {
            self.sections = at(0);
            self.sections_total = at(1);
            self.columns = at(2);
            self.evicted = at(3);
        } else if entry.starts_with("Mesh: ") {
            self.mesh_queued = at(0);
            self.mesh_in_flight = at(1);
            self.uploads_waiting = at(2);
        }
    }

    fn read_tails(&mut self, tails: &str) {
        for stage in tails.split(", ") {
            let Some((name, value)) = stage.split_once(' ') else {
                continue;
            };
            let (p99, max) = match value.split_once('/') {
                Some((p99, max)) => (p99.parse().ok(), max.parse().ok()),
                None => (None, None),
            };
            if let Some(slot) = self.cpu_p99_ms.slot_mut(name) {
                *slot = p99;
            }
            if let Some(slot) = self.cpu_max_ms.slot_mut(name) {
                *slot = max;
            }
        }
    }

    /// The median of each field over `lines`; a field no line carries stays `None`.
    #[must_use]
    pub fn median_of(lines: &[Self]) -> Self {
        let field = |read: fn(&Self) -> Option<f64>| field_of(lines, read);
        let stages = |read: fn(&Self) -> &Stages| Stages {
            main: field_of(lines, |line| read(line).main),
            extract: field_of(lines, |line| read(line).extract),
            prepare: field_of(lines, |line| read(line).prepare),
            acquire: field_of(lines, |line| read(line).acquire),
            render: field_of(lines, |line| read(line).render),
            cleanup: field_of(lines, |line| read(line).cleanup),
        };
        let mut gpu_ms = BTreeMap::new();
        for name in lines.iter().flat_map(|line| line.gpu_ms.keys()) {
            if gpu_ms.contains_key(name) {
                continue;
            }
            let mut values: Vec<f64> = lines
                .iter()
                .filter_map(|line| line.gpu_ms.get(name).copied())
                .collect();
            if let Some(value) = median(&mut values) {
                gpu_ms.insert(name.clone(), value);
            }
        }
        Self {
            fps: field(|line| line.fps),
            frame_median_ms: field(|line| line.frame_median_ms),
            frame_p99_ms: field(|line| line.frame_p99_ms),
            frame_max_ms: field(|line| line.frame_max_ms),
            physical_width: field(|line| line.physical_width),
            physical_height: field(|line| line.physical_height),
            scale: field(|line| line.scale),
            engine_median_ms: field(|line| line.engine_median_ms),
            engine_p99_ms: field(|line| line.engine_p99_ms),
            engine_max_ms: field(|line| line.engine_max_ms),
            engine_frames: field(|line| line.engine_frames),
            cpu_median_ms: stages(|line| &line.cpu_median_ms),
            cpu_p99_ms: stages(|line| &line.cpu_p99_ms),
            cpu_max_ms: stages(|line| &line.cpu_max_ms),
            terrain_draws: field(|line| line.terrain_draws),
            sky_draws: field(|line| line.sky_draws),
            upload_kb: field(|line| line.upload_kb),
            upload_budget_kb: field(|line| line.upload_budget_kb),
            gpu_ms,
            triangles: field(|line| line.triangles),
            triangles_hidden: field(|line| line.triangles_hidden),
            sections: field(|line| line.sections),
            sections_total: field(|line| line.sections_total),
            columns: field(|line| line.columns),
            evicted: field(|line| line.evicted),
            mesh_queued: field(|line| line.mesh_queued),
            mesh_in_flight: field(|line| line.mesh_in_flight),
            uploads_waiting: field(|line| line.uploads_waiting),
            gpu_memory_mib: field(|line| line.gpu_memory_mib),
        }
    }
}

fn field_of(lines: &[StatsLine], read: impl Fn(&StatsLine) -> Option<f64>) -> Option<f64> {
    let mut values: Vec<f64> = lines.iter().filter_map(read).collect();
    median(&mut values)
}

impl Stages {
    fn slot_mut(&mut self, name: &str) -> Option<&mut Option<f64>> {
        match name {
            "main" => Some(&mut self.main),
            "extract" => Some(&mut self.extract),
            "prepare" => Some(&mut self.prepare),
            "acquire" => Some(&mut self.acquire),
            "render" => Some(&mut self.render),
            "cleanup" => Some(&mut self.cleanup),
            _ => None,
        }
    }
}

/// The numbers of an entry in order; a token counts only when it starts with a digit, so
/// `p99` and `inf` are words.
fn numbers(entry: &str) -> Vec<f64> {
    entry
        .split(|separator: char| " ,()/:x%".contains(separator))
        .filter(|token| token.starts_with(|first: char| first.is_ascii_digit()))
        .filter_map(|token| token.parse().ok())
        .collect()
}

fn strip_ansi(text: &str) -> String {
    let mut plain = String::with_capacity(text.len());
    let mut characters = text.chars();
    while let Some(character) = characters.next() {
        if character != '\u{1b}' {
            plain.push(character);
            continue;
        }
        for code in characters.by_ref() {
            if code.is_ascii_alphabetic() {
                break;
            }
        }
    }
    plain
}

#[cfg(test)]
mod tests {
    use super::*;

    const STARTING_WITH_ANSI: &str = "\u{1b}[2m2026-11-10T09:31:15.465953Z\u{1b}[0m \u{1b}[32m INFO\u{1b}[0m \u{1b}[2mmcrs_client::stats\u{1b}[0m\u{1b}[2m:\u{1b}[0m 0 fps T: inf | Minecraft 26.4 Snapshot 7 (0.5.8/mcrs) | Server: not connected | Engine: no frames yet | CPU: main 0.000 ms, extract 0.000, prepare 0.000 (acquire 0.000), render 0.000, cleanup 0.000 | CPU p99/max: main -, extract -, prepare -, acquire -, render -, cleanup - | Draws: 0 terrain, 0 sky | Upload: 0 KB of 4096 KB | XYZ: 0.500 / 80.00000 / 0.500 | Block: 0 80 0 | Chunk: 0 5 0 [0 0 in r.0.0.mca] | Facing: south (Towards positive Z) (0.0 / 0.0) | - FC: 0 | Section-relative: 00 00 00 | 800x602 physical, scale 1.00 | Display: 800x602 | Apple M3 Max | metal | Tris: 0 (0 hidden behind terrain) | Sections: 0/0 in 0 columns, 0 evicted | Mesh: 0 queued, 0 in flight, 0 uploads waiting | Arena: 0.0% quads, 0.0% models, 0.0% faces, 0.0% groups | Sight lines: 0 sections | View: final | GPU memory: 1.1 MiB";

    const WORLD_WITH_GPU: &str = "2026-11-10T09:32:53.014311Z  INFO mcrs_client::stats: 1282 fps (0.731 ms median, 1.524 p99, 5.19 max over the last second) T: inf | Minecraft 26.4 Snapshot 7 (0.5.8/mcrs) | Server: playing as Player | Dimension: minecraft:overworld | Engine: 1.369 ms median, 2.624 p99, 9.91 max over 1281 frames in the last second | CPU: main 0.611 ms, extract 0.242, prepare 0.321 (acquire 0.084), render 0.255, cleanup 0.015 | CPU p99/max: main 1.19/4.71, extract 0.60/2.40, prepare 0.68/1.70, acquire 0.18/0.53, render 0.59/4.55, cleanup 0.05/0.42 | Draws: 0 terrain, 0 sky | Upload: 4096 KB of 4096 KB | GPU cull sections: 0.100 ms | GPU entity cull: 0.007 ms | GPU entity pose: 0.007 ms | GPU entity expand: 0.005 ms | XYZ: 0.000 / 128.00000 / 0.000 | Block: 0 128 0 | Chunk: 0 8 0 [0 0 in r.0.0.mca] | Facing: south (Towards positive Z) (0.0 / 0.0) | minecraft:overworld FC: 0 | Section-relative: 00 00 00 | 800x602 physical, scale 1.00 | Columns: 1225 resident, radius 16 | Hops p50 ms: spawn 0 ready 2416 sent 0 recv 2102 mesh 203 over 1089 | Display: 800x602 | Apple M3 Max | metal | Tris: 0 (0 hidden behind terrain) | Sections: 10520/11930 in 1225 columns, 0 evicted | Mesh: 0 queued, 0 in flight, 10976 uploads waiting | Arena: 2.5% quads, 4.2% models, 3.8% faces, 3.5% groups | Sight lines: 11978 sections in 0.476 ms | View: final | GPU memory: 544.9 MiB";

    // A real floor line with its GPU entries taken out, as an adapter without timestamp
    // queries outside macOS logs it.
    const FLOOR_WITHOUT_GPU: &str = "2026-11-10T09:34:51.742731Z  INFO mcrs_client::stats: 120 fps (8.363 ms median, 11.762 p99, 11.83 max over the last second) T: inf | Minecraft 26.4 Snapshot 7 (0.5.8/mcrs) | Server: not connected | Engine: 10.062 ms median, 13.283 p99, 13.73 max over 120 frames in the last second | CPU: main 0.969 ms, extract 7.632, prepare 6.730 (acquire 6.381), render 1.057, cleanup 0.041 | CPU p99/max: main 1.85/2.21, extract 10.56/11.30, prepare 10.20/10.32, acquire 7.58/10.02, render 1.93/2.05, cleanup 0.21/0.34 | Draws: 0 terrain, 0 sky | Upload: 0 KB of 4096 KB | XYZ: 0.500 / 80.00000 / 0.500 | Block: 0 80 0 | Chunk: 0 5 0 [0 0 in r.0.0.mca] | Facing: south (Towards positive Z) (26.8 / 5.6) | - FC: 0 | Section-relative: 00 00 00 | 800x602 physical, scale 1.00 | Display: 800x602 | Apple M3 Max | metal | Tris: 0 (0 hidden behind terrain) | Sections: 0/0 in 0 columns, 0 evicted | Mesh: 0 queued, 0 in flight, 0 uploads waiting | Arena: 0.0% quads, 0.0% models, 0.0% faces, 0.0% groups | Sight lines: 0 sections in 0.000 ms | View: final";

    struct Expected {
        fps: f64,
        frame_median_ms: Option<f64>,
        engine_frames: Option<f64>,
        main_ms: Option<f64>,
        render_p99_ms: Option<f64>,
        render_max_ms: Option<f64>,
        sections_total: f64,
        uploads_waiting: f64,
        upload_kb: f64,
        gpu_ms: &'static [(&'static str, f64)],
        gpu_memory_mib: Option<f64>,
    }

    #[test]
    fn parses_the_numbers_of_real_stats_lines() {
        let cases = [
            (
                STARTING_WITH_ANSI,
                Expected {
                    fps: 0.0,
                    frame_median_ms: None,
                    engine_frames: None,
                    main_ms: Some(0.0),
                    render_p99_ms: None,
                    render_max_ms: None,
                    sections_total: 0.0,
                    uploads_waiting: 0.0,
                    upload_kb: 0.0,
                    gpu_ms: &[],
                    gpu_memory_mib: Some(1.1),
                },
            ),
            (
                WORLD_WITH_GPU,
                Expected {
                    fps: 1282.0,
                    frame_median_ms: Some(0.731),
                    engine_frames: Some(1281.0),
                    main_ms: Some(0.611),
                    render_p99_ms: Some(0.59),
                    render_max_ms: Some(4.55),
                    sections_total: 11930.0,
                    uploads_waiting: 10976.0,
                    upload_kb: 4096.0,
                    gpu_ms: &[
                        ("cull sections", 0.1),
                        ("entity cull", 0.007),
                        ("entity expand", 0.005),
                        ("entity pose", 0.007),
                    ],
                    gpu_memory_mib: Some(544.9),
                },
            ),
            (
                FLOOR_WITHOUT_GPU,
                Expected {
                    fps: 120.0,
                    frame_median_ms: Some(8.363),
                    engine_frames: Some(120.0),
                    main_ms: Some(0.969),
                    render_p99_ms: Some(1.93),
                    render_max_ms: Some(2.05),
                    sections_total: 0.0,
                    uploads_waiting: 0.0,
                    upload_kb: 0.0,
                    gpu_ms: &[],
                    gpu_memory_mib: None,
                },
            ),
        ];
        for (text, expected) in cases {
            let line = StatsLine::parse(text).unwrap();
            let gpu: Vec<(&str, f64)> = line
                .gpu_ms
                .iter()
                .map(|(name, ms)| (name.as_str(), *ms))
                .collect();
            assert_eq!(line.fps, Some(expected.fps), "{text}");
            assert_eq!(line.frame_median_ms, expected.frame_median_ms, "{text}");
            assert_eq!(line.engine_frames, expected.engine_frames, "{text}");
            assert_eq!(line.cpu_median_ms.main, expected.main_ms, "{text}");
            assert_eq!(line.cpu_p99_ms.render, expected.render_p99_ms, "{text}");
            assert_eq!(line.cpu_max_ms.render, expected.render_max_ms, "{text}");
            assert_eq!(line.physical_width, Some(800.0), "{text}");
            assert_eq!(line.physical_height, Some(602.0), "{text}");
            assert_eq!(line.sections_total, Some(expected.sections_total), "{text}");
            assert_eq!(
                line.uploads_waiting,
                Some(expected.uploads_waiting),
                "{text}"
            );
            assert_eq!(line.upload_kb, Some(expected.upload_kb), "{text}");
            assert_eq!(gpu, expected.gpu_ms, "{text}");
            assert_eq!(line.gpu_memory_mib, expected.gpu_memory_mib, "{text}");
        }
        assert_eq!(
            StatsLine::parse("2026-11-10T09:32:53Z  INFO bevy_render::renderer: AdapterInfo"),
            None,
            "a line of another target is not a stats line"
        );
    }

    #[test]
    fn median_of_takes_each_field_over_the_lines_that_carry_it() {
        let line = |fps: f64, gpu: Option<f64>| StatsLine {
            fps: Some(fps),
            gpu_ms: gpu
                .map(|ms| ("terrain".to_owned(), ms))
                .into_iter()
                .collect(),
            ..StatsLine::default()
        };
        let odd = [
            line(30.0, Some(2.0)),
            line(10.0, None),
            line(20.0, Some(4.0)),
        ];
        let odd = StatsLine::median_of(&odd);
        assert_eq!(odd.fps, Some(20.0));
        assert_eq!(odd.gpu_ms.get("terrain"), Some(&3.0));
        assert_eq!(odd.frame_median_ms, None, "no line carries it");
        let even = [
            line(4.0, None),
            line(1.0, None),
            line(3.0, None),
            line(2.0, None),
        ];
        let even = StatsLine::median_of(&even);
        assert_eq!(even.fps, Some(2.5));
        assert!(even.gpu_ms.is_empty(), "no line carries a GPU slot");
    }
}
