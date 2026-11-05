# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Added `CeichasTimer`, one OS timer waiting until a wall-clock time.

  `wait_until` answers once the wall clock reaches the time, counting suspended time and following clock changes, or once a waker rings, a pending wake winning over a reached time.

- Added `CeichasTimerOptions`, naming the backend a timer waits on.
- Added `CeichasWait`, how a wait ended: `Reached` or `Woken`.
- Added `CeichasWaker`, ending the current or the next wait of its timer from any thread.
- Added `CeichasBackend`: `Timerfd` on Linux and Android, `WaitableTimer` on Windows, `Dispatch` on macOS, `Polling` everywhere.
- Added `CeichasError`, naming the OS call that failed.
- Added the `ceichas` binary behind the `cli` feature, waiting until an RFC 3339 datetime, a duration from now or a UNIX time, then exiting 0; `--verbose` prints how the wait ended.

[Unreleased]: https://github.com/pimalaya/ceichas/compare/root..HEAD
