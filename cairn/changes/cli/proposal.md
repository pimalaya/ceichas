---
cairn: change
id: cli
status: active
created: 2026-11-05
---

# A CLI waiting until a wall-clock time

## Why

The check table marks suspend on macOS and Windows expected, and those checks are the user's, by hand, on machines without a Rust toolchain. examples/at.rs needs cargo to run and takes only a UNIX time or a number of seconds. A binary downloaded from CI, taking a datetime and printing how its wait ended, makes those checks a download and one command. It is also the shell tool the bootstrap left for later: a `sleep` until a time, counting a suspend and following clock changes, where `sleep`, `timeout` and their kin count a duration on a clock that stops during a suspend.

## What

- A `ceichas` binary behind a default-off `cli` feature (`required-features`), std only, no dependency added: `ceichas [-b <backend>] [-v] <TIME>`.
- TIME: an RFC 3339 date and time with `Z` or an offset (seconds and fraction optional, `T` or a space), `+` and a duration in `s`, `m`, `h` and `d` units (`+90`, `+1h30m`), or a UNIX time in seconds. No local time zone: that needs a tz database and DST rules, out until a use asks.
- It ends when the time is reached, exiting 0, and runs no command: the shell chains with `&&`. An error exits 1. Every output goes to stdout, errors included, as in the other Pimalaya CLIs.
- No signal handler and no waker: a signal ends the process the usual way.
- `--verbose` prints the backend and the target before the wait, then the answer time, the lateness and the wall and monotonic times elapsed, which tell a suspend apart.
- A Binaries workflow building it on every push for x86_64 Linux, aarch64 and x86_64 macOS and x86_64 Windows, kept as workflow artifacts; the Tests workflow runs with `--all-features`.

## Left out

- Running a command at the time, a local time zone, a JSON output, clap: none needed for a sleep until a time.
- Publishing the binaries as a release.
