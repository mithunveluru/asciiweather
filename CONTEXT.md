# CONTEXT.md

Working context for asciiweather. Update this whenever the architecture,
feature set, dependencies or known limitations change.

---

## Purpose

A Linux-first terminal weather CLI where real observations are transformed into
procedurally generated ASCII art.

```text
real weather data → normalized model → procedural scene → ASCII → terminal
```

The visual output is a first-class feature, not decoration on an API wrapper.
The engineering underneath must stand on its own.

## Product principles

1. Tiny, fast, delightful, polished, useful, memorable, Linux-native.
2. No API key, no account, no root, no daemon, no telemetry, no shell-outs.
3. The renderer never knows which provider supplied the data.
4. Given a seed, scene generation is deterministic.
5. Core logic is pure and testable without network, filesystem or terminal.
6. Failures degrade gracefully; cached weather keeps the tool useful offline.
7. The default invocation stays extremely simple.
8. Do not build the next feature before the current one is polished.

## Current phase

**MVP complete and polished, plus both stretch phases (forecast, animation).**

Phases 1–19 (inspection → architecture → CLI → model → provider → geocoding →
scenes → renderer → day/night → width tiers → cache → config → output modes →
tests → docs → polish) are done. Phase 20 (forecast) and Phase 21 (animation)
are also done, and were only started once the still image was finished.

## Architecture

```text
Open-Meteo
    │
    ▼
Provider Client   weather/open_meteo.rs   ← only file that knows the API exists
    │
    ▼
WeatherData       weather/model.rs
    │
    ▼
Scene Generator   scene/generator.rs      ← "what should this weather look like?"
    │
    ▼
SceneModel        scene/model.rs
    │
    ▼
Renderer          render/ascii.rs         ← "how should this scene be drawn?"
    │
    ▼
stdout
```

Supporting: `cli.rs` (parsing + glue), `config.rs`, `weather/cache.rs`,
`error.rs`.

Hard rules:

- Provider JSON types are private to `weather/open_meteo.rs`. WMO codes never
  escape it.
- The generator never picks a colour and never asks the terminal anything.
- The renderer never inspects `WeatherCondition` to decide *meaning*, only to
  print a label or pick a two-character icon.
- `render/terminal.rs` is the only module that talks to the OS about the
  terminal.

### Source layout

```text
src/
├── main.rs            8 lines: parse, run, report, exit
├── cli.rs             clap definitions, command dispatch, presentation glue
├── config.rs          ~/.config/asciiweather/config.toml
├── error.rs           AppError: headline / hint / detail / exit code
├── weather/
│   ├── model.rs       Location, WeatherData, WeatherCondition, Intensity, Units
│   ├── provider.rs    trait WeatherProvider { geocode, fetch }
│   ├── open_meteo.rs  HTTP, wire types, WMO → condition + intensity
│   ├── cache.rs       Entry, Cache, atomic write, humanize_age
│   └── mod.rs         resolve(): the cache ↔ provider decision
├── scene/
│   ├── model.rs       SceneModel (7 layers), Sprite, Style, Tier
│   ├── primitives.rs  Glyphs + sun/moon/cloud/bolt/fog/wind/horizon art
│   ├── generator.rs   cloud_plan, celestial, clouds, stars, precipitation,
│   │                  lightning, fog, wind, settle
│   └── rng.rs         SplitMix64
└── render/
    ├── ascii.rs       composite → spans, frame, caption, details, forecast
    ├── color.rs       Paint, Span, ANSI, should_colorize
    ├── terminal.rs    size detection, is_tty, resolve_width
    └── animation.rs   frame loop + TerminalGuard
```

## Implemented features

- `asciiweather <place>` — geocode, fetch, render a procedural scene.
- Configured default location (`asciiweather` with no argument).
- Conditions: Clear, PartlyCloudy, Cloudy, Fog, Drizzle, Rain, HeavyRain,
  Snow, HeavySnow, Thunderstorm, Unknown.
- Intensity (Light/Moderate/Heavy) drives droplet glyph, density and cloud size.
- Day/night from the location's sunrise/sunset — sun ↔ moon, stars at night,
  and a guaranteed minimum star count so night never renders identically to day.
- Wind: slants precipitation in the true blow direction above 20 km/h, adds
  sky streaks, doubles them above 45 km/h.
- Rain and snow fall out of the clouds that are actually drawn, not out of
  clear sky.
- Lightning strikes clear a halo in the rain so the bolt stays legible.
- Dry skies are vertically settled and use a shorter canvas — no dead space.
- Tiers: Compact (≤35 cols), Normal (36–59), Wide (≥60). Labels abbreviate
  below 18 columns. Output is truncated at the frame as a hard guarantee.
- Deterministic seeds; default seed = FNV of location name + observation time.
- Filesystem cache with configurable TTL, atomic writes, offline fallback and
  an age notice.
- `config`, `config set <key> <value>`, `config path`.
- Modes: `--compact`, `--plain`, `--no-color`, `--json`, `--width`, `--seed`,
  `--no-cache`, `--forecast`, `--animate`, `--debug`.
- `NO_COLOR`, `TERM=dumb` and non-TTY stdout all disable colour.
- Five-day forecast riding on the same request.
- Animation: falling precipitation, drifting clouds, twinkling stars, strobing
  lightning; alternate screen, raw mode, restores on any exit including panic.

## Important decisions

Full text in `docs/decisions.md` (ADR-001 … ADR-011). The load-bearing ones:

- **ADR-003** provider abstraction, so Open-Meteo cannot leak.
- **ADR-004** `SceneModel` splits weather semantics from drawing.
- **ADR-005** pinned SplitMix64, not `rand`, so seeded output is stable forever.
- **ADR-006** filesystem cache, not SQLite.
- **ADR-009** `ureq` instead of `reqwest` + `tokio` — two sequential requests
  do not justify an async runtime.
- **ADR-010** no `chrono`; ISO-8601 strings compare lexicographically and
  Sakamoto's algorithm covers weekday names.
- **ADR-011** `--plain` also means "ASCII only", instead of a second flag.

## Dependencies

| Crate | Why |
| --- | --- |
| `clap` (derive) | argument parsing |
| `serde` + `serde_json` | wire types, cache entries, `--json` |
| `toml` | config file |
| `dirs` | XDG config/cache paths |
| `ureq` (rustls, json) | blocking HTTP |
| `crossterm` | terminal size, raw mode, alternate screen, key events |

Deliberately **not** used: `tokio`, `reqwest`, `rand`, `chrono`, `anyhow`,
`terminal_size`, `ctrlc`, SQLite. Each was either replaced by ~15 lines of
std or unnecessary once the async runtime was dropped.

## Known limitations

- Only Open-Meteo is implemented. The trait exists but has one implementation.
- City names are requested in English (`language=en`); wide CJK glyphs would
  mis-measure the caption width if a name came back in a wide script.
- `config set` rewrites the TOML file, so hand-written comments are lost.
- Sunrise/sunset are taken from the observation day only. During polar day or
  polar night the provider omits them and the scene falls back to "day".
- Cache entries are keyed by the query string, so `Chennai` and
  `Chennai, India` are separate entries.
- No hourly forecast; `--forecast` is daily max/min plus rain chance.
- The scene assumes a monospaced terminal and a font with basic box-drawing
  characters. `--plain` is the escape hatch.
- Animation redraws the whole panel each frame (no diffing). At ~8 fps and
  ~50×20 cells this is immaterial, but it is not a general TUI engine.

## Commands

```sh
cargo build --release
cargo test
cargo clippy --all-targets
cargo fmt
cargo run --example gallery -- 52      # every condition, day and night
UPDATE_GOLDEN=1 cargo test             # re-record the one golden file
```

```sh
asciiweather Chennai
asciiweather "New York" --forecast
asciiweather Chennai --json | jq .temperature
asciiweather config set location Chennai
```

## Testing status

**61 tests, all passing. `cargo clippy --all-targets` is clean. No test
touches the network.**

| Suite | Covers |
| --- | --- |
| `src/weather/open_meteo.rs` | geocoding + forecast parsing from fixtures, WMO mapping, query encoding |
| `src/weather/cache.rs` | freshness, expiry, backwards clock, slugs, round-trip, age wording |
| `src/config.rs` | defaults, documented file, partial file, invalid file, `set` validation, TOML round-trip |
| `src/scene/rng.rs` | reproducibility, divergence, bounds |
| `src/scene/primitives.rs` | art bounds, size ordering, exact band widths, ASCII purity |
| `src/render/color.rs` | ANSI on/off, reset per span, padding trim, night palette |
| `src/render/terminal.rs` | explicit vs auto width, clamping |
| `src/render/ascii.rs` | never overflows any width, closed frame, plain output, caption facts, compact one-liner, weekday algorithm |
| `src/cli.rs` | clap definition, positional vs subcommand, flag parsing, error copy |
| `tests/scene.rs` | all 11 conditions × day/night, vocabulary per condition, day ≠ night, intensity density, wind slant + direction, seed determinism and variation, animation frames, narrow vs wide tiers, ASCII purity, one golden file |
| `tests/offline.rs` | fresh cache skips network, expiry refetches, `--no-cache`, outage + cache → notice, outage without cache → exit 4, unknown location → exit 3, JSON round-trip |

Manually verified against the live API: Chennai, London, Tokyo, "New York";
`--compact`, `--plain`, `--no-color`, `--json`, `--forecast`, `--seed 42`
(twice, identical), `--width 20/30/50/52/100`, `config`, `config set`, bare
invocation, unknown location (exit 3), simulated network failure with a warm
cache (notice + scene) and a cold cache (exit 4), `--debug`, `NO_COLOR`, piped
output (no ANSI), and `--animate` under a pty.

## Rejected on purpose

No web dashboard, REST server, auth, cloud database, user accounts, telemetry,
analytics, AI/LLM summaries, Kubernetes, required Docker, plugin framework,
event bus, message queue, microservices, SQLite, background daemon, shell
hooks, or command execution. No IP geolocation. No `--ascii` flag (folded into
`--plain`). No per-condition hardcoded pictures.

## Next steps

Nothing is required for the MVP. If the project grows, in rough order of value:

1. Hourly forecast strip (`--hourly`), one sparkline row of temperature.
2. A second provider behind the existing trait, to prove ADR-003 in anger.
3. Shell completions generated from the clap definition.
4. Frame diffing in the animation loop if anyone runs it on a very large
   terminal.
5. Width measurement that accounts for wide (CJK) glyphs in place names.
