# Reproduce the benchmarks on your machine

This guide is for testers who run the mcrs benchmarks on their own Mac or Windows PC and send the results back. You need no Rust or Minecraft knowledge. You run two scripts, wait, and send one zip file.

## What is measured

The kit in `tools/perf-kit/` runs four sections: the mob simulation benchmark (`mob`), the client rendering a fixed scene (`client`), the server tick (`server`) and world generation (`worldgen`).

Results from different machines are comparable because every run uses the same inputs: the world seed is fixed at 12345, the client window has a fixed size chosen from a short list (1920x1080, 1600x900, 1280x720), and the run follows one of two fixed profiles. The `quick` profile is the default and takes about 45-90 minutes. The `full` profile measures for longer and takes about 3-5 hours.

Next to the numbers, the kit records what the numbers depend on: the CPU, GPU, memory and displays (`system.json`), and the state of the machine before and after the run (`hygiene.json`, `hygiene-after.json`): which apps were open, whether the machine ran on battery, how busy the CPU was. A result from a noisy machine is still useful when we know it was noisy.

## Before you start

- Plan the time: 45-90 minutes for `quick`, 3-5 hours for `full`, plus 30-60 minutes for the first setup and build.
- Plug in the charger. A laptop on battery runs slower and the results do not compare.
- Use a display that stays on for the whole run. The client must render into a visible window: a covered, minimised or switched-off window produces no frames.
- Sit at the machine itself. Do not run the kit over Remote Desktop: on Windows an RDP session changes how the GPU presents frames.
- The first run needs internet access for about 3 GB of downloads (the Rust toolchain, Java, the game data). Later runs only need it for updates.
- Keep 40 GB of free disk space for the toolchain and the build.

## macOS

1. Open the Terminal app (Applications > Utilities > Terminal).
2. If you do not have the repository yet, download only the bootstrap script and run it; it clones the repository into `~/mcrs-perf/mcrs`:

   ```sh
   curl -fsSLO https://raw.githubusercontent.com/andreypfau/mcrs/main/tools/perf-kit/bootstrap.sh
   bash bootstrap.sh
   ```

   If you already have a checkout, run `tools/perf-kit/bootstrap.sh` from it.
3. The bootstrap installs what is missing: the Xcode Command Line Tools (macOS asks for your password), rustup, the pinned Rust toolchain and Java 25. It then builds the release binaries. At the end it prints a table with OK, FIXED or FAILED for each step and a hint for each failure. Run it again after you fix a failure; finished steps are skipped.
4. Start the benchmarks:

   ```sh
   ~/mcrs-perf/mcrs/tools/perf-kit/run.sh
   ```

   Add `--full` for the full profile. The script asks for a machine label (for example `m2-air`) and whether you accept the Minecraft EULA, then walks you through closing apps and checks the machine.
5. When the game window opens, do not touch the computer until the script prints `Done`.

## Windows

1. Open the repository folder in File Explorer, or download `bootstrap.cmd` and `bootstrap.ps1` from `tools/perf-kit/` into one folder if you do not have the repository yet. The bootstrap then clones it into `C:\mcrs`.
2. Double-click `bootstrap.cmd`. If Windows SmartScreen shows "Windows protected your PC", click "More info", then "Run anyway". You can also run it from Windows Terminal or PowerShell:

   ```powershell
   powershell -NoProfile -ExecutionPolicy Bypass -File tools\perf-kit\bootstrap.ps1
   ```

   `-ExecutionPolicy Bypass` applies to this one call only and changes no system setting.
3. The bootstrap installs what is missing with winget: Git, the Visual Studio C++ Build Tools (about 5 GB, asks for administrator rights) and rustup with the pinned toolchain. It downloads Java 25 into a folder of its own. At the end it prints a table with OK, FIXED or FAILED for each step and a hint for each failure. Run it again after you fix a failure.
4. Double-click `tools\perf-kit\run.cmd`, or run `powershell -NoProfile -ExecutionPolicy Bypass -File tools\perf-kit\run.ps1`. Add `-Full` for the full profile.
5. When the game window opens, do not touch the computer until the script prints `Done`.

## What the scripts change and how it is restored

| Change | When | Restored |
| - | - | - |
| Installs Xcode Command Line Tools, rustup, the Rust toolchain (macOS) | bootstrap, if missing | No; remove with `rustup self uninstall` |
| Installs Git, Visual Studio Build Tools, rustup, the Rust toolchain (Windows) | bootstrap, if missing | No; remove them in Settings > Apps |
| Puts Java 25 into `~/.mcrs-perf/jdk` or `%USERPROFILE%\.mcrs-perf\jdk` | bootstrap, if no Java 25 is found | No; delete the folder |
| Sets `git config --global core.longpaths true` (Windows) | bootstrap, only if you answer y; otherwise only the checkout gets it | No |
| Sets `RUSTUP_HOME=C:\rust\rustup` and `CARGO_HOME=C:\rust\cargo` for your user (Windows) | bootstrap, if your profile path has spaces or non-ASCII characters and you answer y; otherwise the scripts set them for their own window | No |
| Adds a Defender exclusion for the repository folder (Windows) | bootstrap, only if you answer y | No; remove with `Remove-MpPreference -ExclusionPath <folder>` as administrator |
| Closes the apps you agree to close | prepare, asked per app | No; start them again after the run |
| Switches the power plan to High performance (Windows) | prepare, only if you answer y | Yes, at the end of run, also after Ctrl+C; if a run stops before that, the next run offers to restore it |
| Keeps the machine and the display awake | run | Yes, when run ends |
| Stops a running Time Machine backup with `tmutil stopbackup` (macOS) | prepare, only if you answer y | No; Time Machine resumes at the next scheduled backup |

The scripts never close the terminal that runs them, nor system processes such as Defender, Windows Update or Spotlight; for those they wait until they calm down and record what they saw. The one exception is a running Time Machine backup, which prepare offers to stop.

## Troubleshooting

| Symptom | Cause | What the script does / what to do |
| - | - | - |
| `link.exe not found` or `linker 'link.exe' not found` | The Visual Studio C++ Build Tools are missing | bootstrap offers to install them; answer y and accept the UAC prompt, then rerun bootstrap |
| `os error 5` or `Access is denied` during the build | An antivirus scan holds a file the build just wrote | The build retries once. If it fails again, rerun bootstrap and accept the Defender exclusion |
| rustup or Java download fails | No internet, or a proxy | bootstrap checks the download hosts first. Behind a proxy, set `HTTPS_PROXY` (`export HTTPS_PROXY=http://host:port` on macOS, `$env:HTTPS_PROXY = "http://host:port"` in PowerShell) and rerun |
| bootstrap stops at `disk` | Less than 40 GB free | Free space on that drive and rerun |
| Build errors with odd paths, or rustup fails to install | Your Windows profile path has spaces or non-ASCII characters | bootstrap moves rustup to `C:\rust` and clones into `C:\mcrs` automatically |
| bootstrap reports `winget FAILED` | App Installer is missing (older Windows 10) | Install "App Installer" from the Microsoft Store, then rerun |
| `java FAILED` or Java too old | Java older than 25 on the PATH | bootstrap ignores it and downloads Java 25 into its own folder |
| The client section has no frames or very few | The window was covered, minimised or on a display that turned off | Rerun and leave the machine alone; keep the window visible and on top |
| GPU lines are missing in the report | The GPU or driver does not support timestamp queries | Nothing to do; CPU-side numbers are still valid. Mention the GPU when you send the results |
| `power source: battery` in hygiene.json | The laptop was not plugged in | Plug in and rerun |
| `thermal state` is not nominal | The machine is throttling | Let it cool down, keep the vents free, and rerun |
| Notifications appeared during the run | Do Not Disturb or Focus was off | Turn it on before the run; prepare asks you to confirm |
| `background services` or `cpu idle gate` failed | Defender, Windows Update, Spotlight or another app kept the CPU busy | prepare waits up to 5 minutes. If it still fails, wait until the machine is idle and rerun |

## The results

When the run ends the script prints the path of a zip file in `tools/perf-kit/results/`, named `<label>-<os>-<UTC date>.zip`. It holds:

- `system.json`: CPU, GPU, memory, displays, power settings, OS, toolchain versions and the repository commit. It holds no user name, host name or serial number.
- `hygiene.json` and `hygiene-after.json`: the machine state before and after the run, with each check, its value and what was done about it.
- `report.json`: the benchmark results.
- `raw/`: the raw measurements behind the report.
- `run.log`: the console output of the benchmark runner.

Send the zip file to the person who asked you to run the benchmarks, with a sentence about anything unusual during the run.
