use std::num::NonZeroU32;

#[cfg(all(feature = "singleplayer", not(target_family = "wasm")))]
use mcrs_minecraft_menu_key::menu_keys::MenuType;
use mcrs_minecraft_mesh::DrawKind;
use mcrs_minecraft_render::sky::SkyEffects;

use crate::config::{Knobs, reject};
#[cfg(all(feature = "singleplayer", not(target_family = "wasm")))]
use crate::dev::join_menu;

impl Knobs {
    #[cfg(all(feature = "singleplayer", not(target_family = "wasm")))]
    pub fn join_menu_server(&self) -> Option<fn(&mut bevy::app::App)> {
        let spec = self.knob("MENU")?;
        MenuType::read(spec.trim())
            .and_then(join_menu::join_menu_server)
            .or_else(|| {
                reject(
                    "MENU",
                    spec,
                    "expected a generic menu type such as generic_9x3 or hopper",
                )
            })
    }

    pub fn play_at_start(&self) -> bool {
        self.flag("PLAY", false)
    }

    pub fn hot_clocks(&self) -> bool {
        self.flag("HOT", false)
    }

    pub fn frame_latency(&self) -> Option<NonZeroU32> {
        self.parsed("LATENCY", |_| true, "expected a frame count above zero")
    }

    pub fn monitor_primary(&self) -> bool {
        self.knob("MONITOR").map(str::trim) == Some("primary")
    }

    pub fn census_interval(&self) -> Option<f32> {
        self.knob("CENSUS")
            .map(|spec| spec.trim().parse().unwrap_or(1.0))
    }

    pub fn tracy_preview(&self) -> bool {
        self.flag("TRACY_PREVIEW", false)
    }

    pub fn chunk_guard(&self) -> Option<Guard> {
        guard(self.knob("CHUNK_GUARD"))
    }

    pub fn light_guard(&self) -> Option<Guard> {
        guard(self.knob("LIGHT_GUARD"))
    }

    pub fn gputrace_path(&self) -> Option<String> {
        self.knob("GPUTRACE").map(str::to_owned)
    }

    pub fn raster_fraction(&self) -> Option<f32> {
        let spec = self.knob("RASTER")?;
        match spec.trim().parse::<f32>() {
            Ok(fraction) if (0.0..=1.0).contains(&fraction) && fraction > 0.0 => Some(fraction),
            _ => {
                eprintln!("MCRS_RASTER takes a fraction between 0 and 1");
                None
            }
        }
    }

    pub fn draw_mask(&self) -> Option<u32> {
        let spec = self.knob("DRAWS")?;
        let mut mask = 0;
        for name in spec.split(',') {
            match name.trim().parse::<u32>() {
                Ok(draw) if (draw as usize) < DrawKind::COUNT => mask |= 1 << draw,
                _ => eprintln!("MCRS_DRAWS takes draw numbers 0..{}", DrawKind::COUNT - 1),
            }
        }
        Some(mask)
    }

    pub fn sky_draws_only(&self) -> Option<SkyEffects> {
        let list = self.knob("SKY")?;
        match SkyEffects::parse(list) {
            Ok(effects) => Some(effects),
            Err(error) => reject("SKY", list, error),
        }
    }

    pub fn scripted_flight(&self) -> Option<f32> {
        self.parsed(
            "FLY",
            |&speed| speed > 0.0,
            "expected a flying speed above zero",
        )
    }

    pub fn turn_after(&self) -> Option<f32> {
        self.parsed(
            "TURN",
            |&seconds| seconds > 0.0,
            "expected seconds above zero",
        )
    }
}

fn guard(value: Option<&str>) -> Option<Guard> {
    match value.map(str::trim) {
        None | Some("0") => None,
        Some("warn") => Some(Guard::Warn),
        Some(_) => Some(Guard::Panic),
    }
}

#[derive(Copy, Clone, PartialEq, Eq)]
pub enum Guard {
    Warn,
    Panic,
}
