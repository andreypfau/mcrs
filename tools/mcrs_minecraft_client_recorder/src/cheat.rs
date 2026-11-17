use std::str::FromStr;

use anyhow::bail;
use glam::DVec3;
use mcrs_minecraft_protocol_game::MoveFlags;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Module {
    Fly,
    Speed,
    NoFall,
    Phase,
    Step,
    Timer,
}

const SPEED: f64 = 1.5;
const TIMER_PERIOD: u32 = 4;

impl FromStr for Module {
    type Err = anyhow::Error;

    fn from_str(text: &str) -> anyhow::Result<Self> {
        Ok(match text {
            "fly" => Self::Fly,
            "speed" => Self::Speed,
            "no_fall" => Self::NoFall,
            "phase" => Self::Phase,
            "step" => Self::Step,
            "timer" => Self::Timer,
            _ => bail!("no cheat module `{text}`"),
        })
    }
}

pub(crate) struct Cheat {
    module: Module,
    last: Option<(DVec3, DVec3)>,
    ticks: u32,
}

impl Cheat {
    pub(crate) fn new(module: Module) -> Self {
        Self {
            module,
            last: None,
            ticks: 0,
        }
    }

    pub(crate) fn teleported(&mut self) {
        self.last = None;
    }

    pub(crate) fn rewrite(&mut self, name: &str, original: &[u8]) -> Option<Vec<u8>> {
        let with_position = matches!(name, "move_player_pos" | "move_player_pos_rot");
        if !with_position && !matches!(name, "move_player_rot" | "move_player_status_only") {
            return None;
        }
        let mut body = original.to_vec();
        let flags = body.last_mut()?;
        let mut parsed = MoveFlags::from_bits(*flags);
        if self.module == Module::NoFall {
            parsed.set_on_ground(true);
            *flags = parsed.into_bits();
        }
        if with_position && let Some(xyz) = body.get_mut(..24) {
            let (xyz, _) = xyz.as_chunks_mut::<8>();
            let client = DVec3::from_array([0, 1, 2].map(|axis| {
                xyz.get(axis)
                    .map_or(0.0, |bytes| f64::from_be_bytes(*bytes))
            }));
            let sent = match (self.module, self.last) {
                (Module::Fly, Some((_, last))) => client.with_y(client.y.max(last.y)),
                (Module::Speed, Some((previous, last))) => {
                    let step = (client - previous) * SPEED;
                    DVec3::new(last.x + step.x, client.y, last.z + step.z)
                }
                (Module::Phase, _) => client - DVec3::Y * 0.5,
                (Module::Step, _) if parsed.on_ground() && parsed.horizontal_collision() => {
                    client + DVec3::Y
                }
                _ => client,
            };
            self.last = Some((client, sent));
            for (bytes, value) in xyz.iter_mut().zip(sent.to_array()) {
                *bytes = value.to_be_bytes();
            }
        }
        (body != original).then_some(body)
    }

    pub(crate) fn repeats_tick(&mut self) -> bool {
        self.ticks += 1;
        self.module == Module::Timer && self.ticks.is_multiple_of(TIMER_PERIOD)
    }
}
