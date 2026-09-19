# Architecture decisions

Short entries. One decision each, with the reason it was worth making.

---

## ADR-001 — Rust for the CLI

The repository was empty. A weather CLI wants a single dependency-free binary,
instant startup, and no runtime to install. Rust gives all three, plus a type
system strong enough to make the layer boundaries below actually enforceable.

## ADR-002 — Open-Meteo as the default provider

Free, no API key, no account, generous for personal use, and it covers
everything needed: geocoding, current conditions, sunrise/sunset and a daily
forecast. Requiring an API key would have made "install and run" a two-step
process for no benefit.

## ADR-003 — A provider abstraction, even with one provider

`WeatherProvider` has exactly one implementation today. It is not speculative
generality: it is the seam that keeps Open-Meteo's URLs, JSON field names and
WMO code numbers inside `weather/open_meteo.rs`. Everything above that file
speaks `Location` and `WeatherData`. Changing providers is a one-file job.

## ADR-004 — `SceneModel` separates weather semantics from rendering

The generator answers *"what should this weather look like?"* and produces a
`SceneModel`: layers of positioned sprites with semantic styles (`Style::Rain`,
`Style::Moon`). The renderer answers *"how should this scene be drawn?"* —
compositing, framing, ANSI. The generator never picks a colour or asks how wide
a terminal is; the renderer never asks what the weather means.

The practical payoff is testing: the entire visual pipeline runs with no
network, no filesystem and no terminal.

## ADR-005 — Deterministic seeds make procedural rendering testable

Scene generation uses a pinned SplitMix64 (`scene/rng.rs`) rather than the
`rand` crate, so a seeded scene is byte-identical across machines and crate
updates. `--seed 42` is reproducible; the default seed is derived from the
location name plus the observation timestamp, so repeated runs against the same
reading redraw the same picture and a new reading redraws a new one.

Animation reuses this: frame *n* is the same seeded scene sampled later, not a
second source of randomness.

## ADR-006 — A filesystem cache, not a database

One small JSON file per query under `~/.cache/asciiweather/`. The working set
is a handful of entries read by a short-lived process; SQLite would add a
dependency, a schema and a migration story to solve a problem that does not
exist. Writes go to a temp file and are renamed, so an interrupted run cannot
corrupt an entry, and an unparseable entry is simply ignored.

The cache is keyed by the query string rather than by coordinates, so an
offline run can skip geocoding as well as the forecast request.

## ADR-007 — Static rendering is the MVP; animation is secondary

Everything — day/night, intensity, wind, tiers, colour, offline — was finished
and tested against the still image first. `--animate` is a thin loop over the
existing generator with a frame counter, not a parallel rendering path.

## ADR-008 — No telemetry and no automatic IP geolocation

Silently resolving the user's location from their IP address is a network call
they did not ask for, about a fact they did not volunteer. The location comes
from the command line or from a config file the user wrote. There is no
telemetry, no analytics and no background process.

## ADR-009 — `ureq` instead of `reqwest` + `tokio`

The program makes at most two sequential HTTP requests and then exits. An async
runtime buys nothing here and costs a large dependency tree and a colour on
every function in the call chain. `ureq` is blocking, small, and rustls-backed,
so the `WeatherProvider` trait stays a plain synchronous trait that a test
double can implement in five lines.

Reconsider if the tool ever needs concurrent requests — it does not yet.

## ADR-010 — No date/time library

Open-Meteo returns local ISO-8601 timestamps with identical shapes
(`2026-08-20T14:15`), so comparing "now" against sunrise and sunset is a string
comparison that is also a correct chronological comparison. Cache ages come
from `SystemTime` arithmetic on Unix seconds, and weekday names from Sakamoto's
algorithm in six lines. `chrono` would be a large dependency for three small
jobs.

## ADR-011 — `--plain` doubles as the ASCII-only mode

Rather than adding a separate `--ascii` flag, `--plain` — already the "minimal
terminal, SSH, pipes" mode — also restricts the art to pure ASCII. The two
audiences are the same audience, and the CLI stays smaller.
