use std::fs::File;
use std::io::Write;
use std::path::Path;

pub(crate) fn write_atomic(path: &Path, contents: &str) -> std::io::Result<()> {
    let mut temp = path.as_os_str().to_owned();
    temp.push(".tmp");
    let mut file = File::create(&temp)?;
    file.write_all(contents.as_bytes())?;
    // Without the sync a power loss can keep the rename and lose the data.
    file.sync_all()?;
    drop(file);
    std::fs::rename(&temp, path)
}
