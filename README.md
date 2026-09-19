# asciiweather

**Weather, rendered by your terminal.**

```console
$ asciiweather Chennai
╭──────────────────────────────────────────────────╮
│                                                  │
│                                .---.             │
│               .---.         .-(     ).           │
│            .-(     ).      (____._____)          │
│           (____._____)                           │
│                │                  │ │            │
│              │      │         │    │   │         │
│          ││ │       │          │   │  │          │
│             ││             │      │  │           │
│         │    │           │     │        │        │
│                  │              │                │
│  ______________________________.--.________._._  │
│                                                  │
│                       27°C                       │
│                  Chennai, India                  │
│                       Rain                       │
│                                                  │
│         Feels like 33°C  ·  Humidity 87%         │
│           Wind 9 km/h SW  ·  Rain 72%            │
│                                                  │
╰──────────────────────────────────────────────────╯
```

Real observations from [Open-Meteo](https://open-meteo.com), turned into a
procedurally generated ASCII scene — not a stock photo of a raincloud. No API
key, no account, no daemon, no telemetry. One binary, one HTTP request, one
picture. That's the whole pitch.

---

## Features

- **Procedural scenes, not clip art.** Sun, moon, clouds, rain, snow, fog,
  wind and lightning are composable primitives. Nothing is a stored picture of
  "rain"; every scene is generated.
- **Day and night.** Derived from the location's own sunrise and sunset, not
  from your system clock. The same weather looks different after dark.
- **Intensity matters.** Drizzle, rain and heavy rain differ in droplet glyph,
  density and cloud size. Same for snow.
- **Wind is visible.** Above ~20 km/h the precipitation slants — in the
  direction the wind is actually blowing — and streaks appear in the sky.
- **Deterministic.** `--seed 42` always produces exactly the same scene.
- **Width aware.** Compact, normal and wide rendering tiers. Never overflows.
- **Works offline.** A short-lived filesystem cache keeps the last reading
  usable when the network is not.
- **Scriptable.** `--json`, `--plain`, `--no-color`, honest exit codes,
  errors on stderr.

<details>
<summary>More scenes</summary>

```text
Clear, night                         Thunderstorm, day
╭────────────────────────────────╮   ╭────────────────────────────────╮
│                                │   │            .---.               │
│    ·                       ·   │   │   .---.  .-(     ).            │
│           .---.                │   │.-(     ).(____._____)          │
│  *       /  .  \               │   │(____.____)                     │
│         |   o   |        ·     │   │      /   ┃    ┃    ┃ ┃ ┃       │
│    ·     \  .  /   ✦   *       │   │     /__      ┃  ┃ ┃   ┃        │
│           `---'                │   │  ┃ ┃  /        ┃   ┃  ┃        │
│                                │   │    ┃ /     ┃      ┃  ┃         │
│  ____________._._____________  │   │  ______/\____._.________.--._  │
╰────────────────────────────────╯   ╰────────────────────────────────╯
```

Run `cargo run --example gallery` to render every condition, day and night.
</details>

## Deliberately absent

No web dashboard, no REST API, no cloud database, no user accounts, no
Kubernetes, no message queue, no plugin framework, no background daemon. It's
a CLI that fetches the weather and draws it. That job doesn't need a control
plane.

## Install

```sh
git clone https://github.com/mithunveluru/asciiweather
cd asciiweather
cargo install --path .
```

Or build it and copy the binary anywhere on your `PATH`:

```sh
cargo build --release
install -m755 target/release/asciiweather ~/.local/bin/
```

Requires a Rust toolchain (2024 edition). No system libraries, no root, no
`docker-compose.yml` summoning six containers to render a cloud.

## Usage

```sh
asciiweather Chennai
asciiweather London
asciiweather "New York"
asciiweather                    # uses the configured location
```

| Option | What it does |
| --- | --- |
| `--compact` | One line instead of a scene |
| `--plain` | No frame, no colour, ASCII-only art — good for SSH and pipes |
| `--no-color` | Keep the frame, drop the ANSI |
| `--json` | Normalized weather data on stdout, nothing else |
| `--width <N>` | Render at N columns instead of detecting |
| `--seed <N>` | Reproducible scene |
| `--no-cache` | Skip the cache, always fetch |
| `--forecast` | Next five days |
| `--animate` | Falling rain, drifting clouds, flashing storms |
| `--debug` | Include technical detail in error messages |

## Location

The positional argument is a free-text place name, resolved through
Open-Meteo's geocoding endpoint:

```sh
asciiweather Reykjavik
asciiweather "São Paulo"
```

There is **no IP-based geolocation**. asciiweather never guesses where you
are — it's not that kind of app. You tell it, once:

```sh
asciiweather config set location Chennai
asciiweather              # from now on, Chennai
```

## Configuration

`~/.config/asciiweather/config.toml`:

```toml
[general]
location = "Chennai"
units = "metric"      # or "imperial"

[render]
color = true
width = "auto"        # or a column count, e.g. "60"

[cache]
ttl_minutes = 10
```

```sh
asciiweather config                        # show current settings
asciiweather config set units imperial
asciiweather config path
```

The file is rewritten by `config set`, so hand-written comments are not
preserved. Everything has a sensible default; a missing file is not an error.

## Cache

Readings are cached under `~/.cache/asciiweather/`, one small JSON file per
query. Writes go to a temporary file and are renamed into place, so an
interrupted run can never leave a corrupt entry.

- Fresh entry (< `ttl_minutes`) → used, no request is made.
- Expired entry → a fresh request is made.
- Request fails but an entry exists → the cached reading is shown, with a notice:

  ```text
  ⚠ No network connection. Showing cached weather from 7 minutes ago.
  ```

- Request fails and nothing is cached → a plain error and a non-zero exit.

The cache key is the query you typed, so an offline run needs no geocoding
either. `--no-cache` bypasses reads (fresh results are still written). Nothing
here phones home; it just remembers what the sky looked like.

## JSON mode

```console
$ asciiweather Chennai --json | jq '{temp: .temperature, sky: .condition}'
{
  "temp": 27.3,
  "sky": "rain"
}
```

stdout carries only JSON. Notices and errors go to stderr.

## Rendering

Colour is an accent, never load-bearing: every scene reads correctly in
monochrome. Colour is enabled only when stdout is a terminal, `NO_COLOR` is
unset, `TERM` is not `dumb`, and neither `--no-color` nor `--plain` was passed.

Width is detected from the terminal and capped to a comfortable card
(52 columns). `--width` takes your number literally. Below ~36 columns the
scene drops decoration and shortens labels rather than wrapping or overflowing.

`--plain` additionally restricts the art to pure ASCII (`|` instead of `│`,
`*` instead of `✦`), which is the safest choice for unusual terminals.

## Forecast

```console
$ asciiweather Chennai --forecast
╭──────────────────────────────────────────────────╮
│                                                  │
│              CHENNAI, INDIA  ·  °C               │
│                                                  │
│            Today ☁'   35° / 26°   73%            │
│            Fri   ☁'   37° / 27°   50%            │
│            Sat   ☁│   36° / 26°   73%            │
│            Sun   ☁│   35° / 26°   88%            │
│            Mon   ☁'   35° / 25°   85%            │
│                                                  │
╰──────────────────────────────────────────────────╯
```

The forecast rides along on the same request as current conditions, so it
costs nothing extra.

## Animation

```sh
asciiweather Chennai --animate      # any key, q, Esc or Ctrl+C to quit
```

Each frame is the *same seeded scene* sampled at a later time: droplets keep
falling, clouds drift, storms strobe, stars twinkle. It runs at roughly
8 frames per second in an alternate screen buffer, and restores the terminal on
exit — including on panic. Even if it crashes, your shell survives; priorities.
Animation is skipped automatically when stdout is not a TTY, or with `--json`,
`--plain` or `--compact`.

## Architecture

```text
        Open-Meteo
             │
             ▼
      Provider Client        weather/open_meteo.rs   ← the only file that
             │                                          knows the API exists
             ▼
        WeatherData          weather/model.rs
             │
             ▼
     Scene Generator         scene/generator.rs      ← "what should this
             │                                          weather look like?"
             ▼
        SceneModel           scene/model.rs
             │
             ▼
     Terminal Renderer       render/ascii.rs         ← "how should this
             │                                          scene be drawn?"
             ▼
          stdout
```

The boundaries are the point. Provider JSON never reaches the renderer; the
renderer never asks what the weather means; the generator never picks a colour
or touches a terminal. Swapping providers means rewriting one file.

```text
src/
├── main.rs            thin binary
├── cli.rs             argument parsing and glue
├── config.rs          config.toml
├── error.rs           user-facing errors and exit codes
├── weather/
│   ├── model.rs       Location, WeatherData, WeatherCondition, Intensity
│   ├── provider.rs    the WeatherProvider trait
│   ├── open_meteo.rs  Open-Meteo client and WMO code mapping
│   ├── cache.rs       atomic filesystem cache
│   └── mod.rs         cache/provider resolution
├── scene/
│   ├── model.rs       SceneModel, Sprite, Style, Tier
│   ├── primitives.rs  sun, moon, cloud, bolt, fog, wind, horizon
│   ├── generator.rs   weather semantics -> scene
│   └── rng.rs         deterministic SplitMix64
└── render/
    ├── ascii.rs       compositing, framing, captions
    ├── color.rs       spans and ANSI
    ├── terminal.rs    width detection
    └── animation.rs   the frame loop
```

## Development

```sh
cargo fmt
cargo clippy --all-targets
cargo test
cargo run --example gallery -- 52     # every condition, day and night
```

### Testing

Tests never touch the network. Provider parsing runs against recorded fixtures
in `tests/fixtures/`, and the cache/offline logic runs against a fake provider.

- `src/**` unit tests — WMO code mapping, cache freshness, config parsing,
  RNG bounds, ANSI emission, CLI argument parsing, error copy.
- `tests/scene.rs` — every condition, day and night, intensity, wind,
  determinism, seed variation, animation frames, width tiers, ASCII purity,
  and one golden file.
- `tests/offline.rs` — fresh cache, expired cache, `--no-cache`, network
  outage with and without cached data, unknown location, JSON round-trip.

The single golden file is regenerated with:

```sh
UPDATE_GOLDEN=1 cargo test
```

## Privacy

- No telemetry, no analytics, no accounts, no background service.
- No IP geolocation. You provide the location.
- No shell-outs; the program never executes another command.
- No root, no setuid, no system-wide state.
- Two outbound requests, both to `open-meteo.com`: one geocode, one forecast.
  Cached runs make none.
- Everything it stores lives in `~/.config/asciiweather/` and
  `~/.cache/asciiweather/`, plain text you can read, edit, or delete without
  asking anyone's permission.

## Licence

MIT. Do what you want with it; just don't blame the weather.
