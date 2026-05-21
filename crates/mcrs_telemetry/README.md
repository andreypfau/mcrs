# mcrs_telemetry

Telemetry substrate for mcrs: Tracy profiling and Bevy diagnostics behind optional cargo features, with zero overhead in default builds.

## SystemSet span coverage

Per-system Tracy span coverage is provided by two complementary mechanisms. The first is Bevy's `bevy_ecs/trace` cargo feature, enabled workspace-wide in `Cargo.toml`. With that feature on, Bevy emits one tracing span per system invocation (`info_span!(parent: None, "system", name = ...)` at `function_system.rs`), one span per schedule run, and one span per set-condition evaluation. These spans appear in every Tracy capture whenever the subscriber is installed.

The second mechanism is a `module::function` naming convention applied to `#[cfg_attr(feature = "telemetry-tracy", tracing::instrument(...))]` attributes at hot system bodies. Examples: `lighting::propagate_decrease`, `lighting::light_converge_driver`, `world::column_gen`, `world::tick_explode::calc_blocks`, `world::tick_explode::deduplicate_blocks`, `network::process_received_packet`. Filtering by these prefixes in the Tracy GUI groups subsystems visually in the same way a per-`SystemSet` wrapper would, and the spans correctly inherit the `dim` attribute set by the enclosing `dim_tick` or `dim_extract` span when running inside a DimSubApp.

A project-supplied wrapper helper that opens a Tracy zone on `SystemSet` entry and closes it on exit was considered but not adopted. Bevy 0.18 exposes no public API to inject a per-set wrapper system that fires on set entry/exit boundaries. `add_systems(..., wrapper.in_set(X))` adds a sibling system inside the set; the resulting Tracy zone is a peer of the set's other systems, not a parent that nests them. The introspection API `ScheduleGraph::system_sets` enumerates registered sets but provides no injection hook — enumeration without injection cannot produce the parent-zone semantics the downstream need was after.

If a future Bevy release exposes a public hook for wrapping a `SystemSet` (a set-enter/set-exit observer, run-condition with start/end callbacks, or equivalent), this strategy will be revisited. Until then, the per-system span plus the `module::function` naming convention is the substrate's `SystemSet` coverage contract. Prefix-based filtering in the Tracy GUI achieves equivalent observability.

## Cargo features

| Feature | Description |
|---------|-------------|
| `telemetry` | Convenience alias that enables both `telemetry-tracy` and `telemetry-diagnostics`. |
| `telemetry-tracy` | Activates `bevy_log/tracing-tracy`, which installs the Tracy subscriber layer inside `bevy_log::LogPlugin`. Per-system and per-schedule spans are emitted automatically by `bevy_ecs/trace`. |
| `telemetry-diagnostics` | Adds `FrameTimeDiagnosticsPlugin` and `EntityCountDiagnosticsPlugin` to the main app so frame-time and entity-count data surface alongside Tracy zones in the same capture. |

None of these features are in `default`. Activation is always explicit (`cargo run --features=telemetry`).

## When telemetry is enabled

With `--features=telemetry-tracy`, `bevy_log::LogPlugin` composes `TracyLayer::default()` into its layered subscriber and calls `set_global_default`. From that point on, every span emitted anywhere in the process reaches the Tracy capture session.

At DimSubApp pump start, `dim_tick` and `dim_extract` spans are opened with a `dim` field (e.g., `dim="overworld"`, `dim="the_nether"`). Spans created by `#[instrument]` attributes at hot system bodies — `lighting::propagate_decrease`, `lighting::light_converge_driver`, `world::column_gen`, `network::process_received_packet`, and others — inherit the active `dim` span as their parent and carry the `dim` field through to the Tracy zone. The Bevy-emitted per-system spans use `parent: None` by design and do not inherit `dim`; filtering by the `dim_tick` parent zone in the Tracy GUI is the ergonomic workaround.

## When telemetry is disabled

Without `telemetry-tracy`, no `tracing-tracy` code is compiled into the binary. No Tracy client connection is attempted, no span data is allocated, and no subscriber is installed. The `#[cfg_attr(feature = "telemetry-tracy", tracing::instrument(...))]` attributes on hot system bodies expand to nothing. The crate's `TelemetryPlugin::build()` body is a no-op.
