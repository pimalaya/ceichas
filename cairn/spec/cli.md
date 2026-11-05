---
cairn: spec
capability: cli
status: current
---

# CLI

The `ceichas` binary, built with the default-off `cli` feature, is a `sleep` until a wall-clock time: it waits on one `CeichasTimer` and exits once the time is reached, the shell chaining what runs then (`ceichas 2026-11-06T08:45:00+01:00 && mpv alarm.ogg`). It depends on std alone, and its `--verbose` output serves the hand checks of the timer check table on machines without a Rust toolchain, through the binaries the Binaries workflow keeps as artifacts.

### Requirement: The binary waits until a time, then exits 0
`ceichas <TIME>`, built with the `cli` feature, SHALL wait on a `CeichasTimer` until TIME and SHALL then exit 0, printing nothing without `--verbose`. It SHALL run no command and install no signal handler. TIME SHALL be an RFC 3339 date and time ending in `Z` or a `±HH:MM` offset, the seconds and their fraction optional and a space allowed in place of the `T`, a leap second reading as the next second; or `+` and a duration made of numbers each followed by a unit among `s`, `m`, `h` and `d`, the last number in seconds when it has none; or a UNIX time in whole seconds. A time already past SHALL exit 0 at once.

#### Scenario: A datetime with an offset
- **GIVEN** the time two seconds ahead as an RFC 3339 datetime with a `+01:00` offset
- **WHEN** `ceichas` waits until it
- **THEN** it exits 0 at most one second after the time

#### Scenario: A duration
- **WHEN** `ceichas +1m30s` runs
- **THEN** it exits 0 ninety seconds later, at most one second late

### Requirement: The backend is chosen by name
`--backend` (`-b`) SHALL take one of `timerfd`, `waitable-timer`, `dispatch` and `polling`, each accepted only on the targets `CeichasBackend::available()` lists it on, the target's default otherwise.

### Requirement: The verbose output tells a suspend apart
With `--verbose` (`-v`), the binary SHALL print, before the wait, a line `waiting` with the backend, the target in RFC 3339 UTC to the millisecond and the milliseconds until it, and after the wait a line `reached` with the answer time, the milliseconds late, and the wall and monotonic milliseconds elapsed.

### Requirement: An error exits 1
An unknown argument, a missing or invalid TIME, a time out of range, a backend unknown on the target and a `CeichasError` SHALL print one capitalised line naming the problem on stdout and exit 1.

#### Scenario: An invalid time
- **WHEN** `ceichas tomorrow` runs
- **THEN** it prints `Invalid time tomorrow, see --help` and exits 1
