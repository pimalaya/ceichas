---
cairn: tasks
change: bootstrap
---

## M0: review

- [x] Proposal and delta reviewed (user, 2026-11-05): `kqueue` and `TFD_TIMER_CANCEL_ON_SET` left out, no slower fallback mode in any backend, `Dispatch` the macOS default, a pending wake winning over a reached time, `CeichasTimerOptions` instead of `with_backend`, `Polling` steps of one second, a one-second lateness bound, native defaults on their OS with a check table, every check CI can run in CI, suspend tested wherever available, the wake opt-in coming with a NixOS module
- [x] The repository pimalaya/ceichas created (user); local repository with the empty signed root commit "init repository" and the `root` tag

## M1: repository

- [x] Cargo.toml (`ceichas`, edition 2024, `MIT OR Apache-2.0`, rust-version as simd-cpu), LICENSE-MIT and LICENSE-APACHE, flake.nix, shell.nix (with the rust-std of the Windows, macOS, iOS, Android and FreeBSD targets for `cargo check --target`), default.nix, package.nix (`[ mit asl20 ]`), deny.toml, README, CHANGELOG, SECURITY.md, .gitignore, after simd-cpu
- [x] src/lib.rs header: the architecture, the guarantees, the backend table

## M2: interface and `Polling`

- [x] `CeichasTimer`, `CeichasTimerOptions`, `CeichasWait`, `CeichasWaker`, `CeichasError`, `CeichasBackend` (`default`, `available`)
- [x] `Polling`: condvar, one-second steps, the kept wake
- [x] tests/timer.rs: the delta's automatable scenarios, run against every backend `available()` lists, each wait behind a deadline

## M3: `Timerfd` (Linux, Android)

- [x] rustix `timerfd` (`CLOCK_REALTIME`, `ABSTIME`), `eventfd`, `poll`, an interrupted `poll` going on

## M4: `WaitableTimer` (Windows)

- [ ] windows-sys waitable timer (absolute UTC `FILETIME` due time, `fResume` false), auto-reset event, `WaitForMultipleObjects`

## M5: `Dispatch` (macOS)

- [ ] libdispatch FFI: source timer at `dispatch_walltime` with `DISPATCH_TIMER_STRICT`, a semaphore for both signals, the source's own semaphore reference released by its cancel handler, the source cancelled and released on drop

## M6: clock and suspend checks

- [ ] tests/clock.rs: clock set forward past a time and back before one, against every backend, ignored, refusing to run without `CEICHAS_CLOCK_TESTS=1`, the clock restored by a guard
- [x] examples/at.rs: waits until a UNIX time or an offset, prints when it answered
- [x] nix/vm-test.nix, the flake's `checks.x86_64-linux.vm`: the datetime, the clock set forward and back as root, a suspend to RAM over a time with a monotonic `sleep` as control, on every Linux backend (passing locally, 2026-11-05)
- [ ] Android: one Termux run of the Linux checks (user)

## M7: CI

- [ ] tests.yml: fmt, clippy and tests on Linux, macOS and Windows; the MSRV build; `cargo clippy --target` for Android, iOS and FreeBSD; the clock job on the three runners; the NixOS VM job with KVM
- [ ] audit.yml (cargo-deny), after simd-cpu

## M8: land

- [ ] Fold the delta into spec/timer.md with the check table, log entry, CHANGELOG
- [ ] Release (user)
- [ ] carillon: the alarm wait swapped to ceichas (carillon change alarm-hooks, last milestone)
