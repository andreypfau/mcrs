use std::collections::HashSet;
use std::ffi::OsString;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;
use std::time::Instant;

use crate::handle::Handle;
use crate::tools::Output;

const PNG_END: [u8; 8] = [0x49, 0x45, 0x4e, 0x44, 0xae, 0x42, 0x60, 0x82];

pub fn capture(handle: &Handle, max_px: u32) -> Result<Output, String> {
    let dir = handle.dir().join("screenshots");
    let before = pngs(&dir);
    std::fs::write(dir.join("capture"), b"")
        .map_err(|error| format!("{}: {error}", dir.display()))?;
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(30) {
        if !handle.alive() {
            return Err(format!("{} exited", handle.id));
        }
        let shot = pngs(&dir)
            .into_iter()
            .find(|name| !before.contains(name))
            .map(|name| dir.join(name));
        if let Some(shot) = shot
            && let Ok(png) = std::fs::read(&shot)
            && png.ends_with(&PNG_END)
        {
            return Ok(scaled(&shot, max_px).unwrap_or(Output::Image { path: shot, png }));
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    Err("no screenshot within 30 s; is the window drawn?".to_owned())
}

fn pngs(dir: &Path) -> HashSet<OsString> {
    std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .filter_map(|entry| Some(entry.ok()?.file_name()))
        .filter(|name| {
            Path::new(name)
                .extension()
                .is_some_and(|extension| extension == "png")
        })
        .collect()
}

fn scaled(shot: &Path, max_px: u32) -> Option<Output> {
    let path: PathBuf = shot.with_file_name(format!(
        "{}-{max_px}.png",
        shot.file_stem()?.to_string_lossy()
    ));
    let status = Command::new("sips")
        .args(["-Z", &max_px.to_string()])
        .arg(shot)
        .arg("--out")
        .arg(&path)
        .output()
        .ok()?;
    let png = std::fs::read(&path)
        .ok()
        .filter(|_| status.status.success())?;
    Some(Output::Image { path, png })
}
