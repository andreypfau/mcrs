# perf-kit

Scripts that set up a macOS or Windows machine, record its state and run the mcrs benchmarks. The full guide for testers is [docs/perf/reproduce.md](../../docs/perf/reproduce.md).

- `bootstrap.sh` / `bootstrap.ps1` (`bootstrap.cmd`): install the toolchain and Java 25, build the release binaries. `--check` / `-Check` only reports.
- `run.sh` / `run.ps1` (`run.cmd`): the one entry point. Asks for the language of its messages (`--lang en|ru` / `-Lang en|ru` skips the question) and a label, records the machine, prepares it, runs `perf-kit run` and zips everything into `results/`.
- `sysinfo.sh` / `sysinfo.ps1`: write `system.json`.
- `prepare.sh` / `prepare.ps1`: hygiene checks, write `hygiene.json`. `--check-only` / `-CheckOnly` records without prompts.
- `common.sh` / `common.ps1`: shared helpers.

The `.sh` scripts target macOS with its bash 3.2; the `.ps1` scripts target Windows PowerShell 5.1.
