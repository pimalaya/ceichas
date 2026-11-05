---
cairn: spec
capability: timer
status: current
---

# Timer

A `CeichasTimer` waits until a wall-clock time on one OS timer. `CeichasTimer::new` takes the timer's OS resources on the backend its `CeichasTimerOptions` name, `CeichasTimer::backend` tells which, `CeichasTimer::wait_until` blocks until a `SystemTime` and answers `CeichasWait::Reached` or `CeichasWait::Woken`, and `CeichasTimer::waker` gives the `CeichasWaker` that ends a wait from any thread. The times to wait for, what runs when one is reached and the threads stay the caller's: a timer waits for one time at once.

### Requirement: A wait ends when the wall clock reaches its time
`CeichasTimer::wait_until` SHALL block until `SystemTime::now()` is at or after the time it is given, and SHALL then answer `CeichasWait::Reached`. It SHALL never answer `Reached` before: a backend's signal SHALL be checked against `SystemTime::now()`, and the wait SHALL go on when the time is not reached. While the machine runs, it SHALL answer at most one second after the time. A time at or before now, a time before the UNIX epoch included, SHALL answer `Reached` without blocking. A time beyond what the OS timer holds SHALL be waited for without error, a wake ending it.

#### Scenario: A datetime
- **GIVEN** a timer
- **WHEN** it waits until a whole-second datetime two seconds ahead
- **THEN** it answers `Reached`, `SystemTime::now()` being then at or after the datetime and at most one second past it

#### Scenario: A time already past
- **GIVEN** a timer
- **WHEN** it waits until one second ago
- **THEN** it answers `Reached` at once

#### Scenario: A time before the UNIX epoch
- **GIVEN** a timer
- **WHEN** it waits until a day before 1970-01-01
- **THEN** it answers `Reached` at once

#### Scenario: Many times in a row
- **GIVEN** a timer
- **WHEN** it waits twenty times in a row until 20 ms ahead
- **THEN** every wait answers `Reached`, never before its time and at most one second after

### Requirement: A waker ends a wait from another thread
`CeichasTimer::waker` SHALL give a `CeichasWaker`, `Clone`, `Send` and `Sync`, whose `wake` SHALL end the wait in progress with `CeichasWait::Woken`. A wake sent while no wait is in progress SHALL end the next wait at once, and the wakes sent before one wait SHALL end that wait alone. A wait finding both its time reached and a wake pending SHALL answer `Woken`, and a wake SHALL be consumed only by a wait answering `Woken`. A waker outliving its timer SHALL wake nothing and answer no error.

#### Scenario: A wake during a wait
- **GIVEN** a thread waiting until an hour from now
- **WHEN** another thread calls `wake`
- **THEN** the wait answers `Woken`

#### Scenario: A wake before the wait
- **GIVEN** a waker woken twice while no thread waits
- **WHEN** a thread waits until an hour from now, then waits again
- **THEN** the first wait answers `Woken` at once, and the second blocks until the next wake

#### Scenario: A wake and a reached time
- **GIVEN** a woken waker
- **WHEN** its timer waits until a past time, then again
- **THEN** the first wait answers `Woken`, and the second `Reached`

#### Scenario: A far-future time
- **GIVEN** a thread waiting until ten thousand years from now
- **WHEN** another thread calls `wake`
- **THEN** the wait answers `Woken`

#### Scenario: A waker outliving its timer
- **GIVEN** a waker whose timer was dropped
- **WHEN** it wakes
- **THEN** it answers no error

### Requirement: Suspended time counts
A time reached while the machine is suspended SHALL end the wait within one second of the process running again.

#### Scenario: A suspend over the time
- **GIVEN** a timer waiting until ten seconds from now
- **WHEN** the machine is suspended for thirty seconds
- **THEN** the wait answers `Reached` within one second of the resume, while a monotonic sleep of ten seconds started beside it ends late by the suspend

### Requirement: The wall clock is followed
A clock set at or past the time SHALL end the wait with `Reached` within one second, and a clock set back SHALL keep the wait going until the new wall time reaches the time.

#### Scenario: A clock set forward
- **GIVEN** a timer waiting until an hour from now
- **WHEN** the clock is set two hours forward
- **THEN** the wait answers `Reached` within one second

#### Scenario: A clock set back
- **GIVEN** a timer waiting until five seconds from now
- **WHEN** the clock is set a minute back
- **THEN** the wait still goes on eight seconds later, and answers `Reached` within one second of the clock being set a minute forward again

### Requirement: A backend is chosen, or the target's default
A timer SHALL wait on the `CeichasBackend` its `CeichasTimerOptions` name: `Timerfd` (a `timerfd` on `CLOCK_REALTIME` armed with an absolute time) on Linux and Android, `WaitableTimer` (a waitable timer given an absolute UTC due time) on Windows, `Dispatch` (a dispatch source timer started at a `dispatch_walltime`) on macOS, and `Polling` (a condition variable waited on in steps of at most one second, the wall clock read at each) on every target where std has threads. A variant SHALL exist only on the targets it serves. `CeichasTimerOptions::default()` SHALL name `CeichasBackend::default()`, the target's native backend, else `Polling`, and `CeichasBackend::available()` SHALL list every backend the target compiles, its default first. Every backend SHALL hold every other requirement of this capability, and a native backend's thread SHALL wake only when its time is reached or a waker rings it. A native backend found failing a requirement on an OS SHALL stop being the default there.

#### Scenario: The default backend
- **GIVEN** Linux
- **WHEN** a timer is built with `CeichasTimerOptions::default()`
- **THEN** `backend()` answers `Timerfd`

#### Scenario: Every backend of a target
- **GIVEN** a target
- **WHEN** the suite runs against each backend `CeichasBackend::available()` lists
- **THEN** every automatable scenario of this capability holds on each

### Requirement: A sleeping machine is never woken
No timer SHALL wake a suspended machine: Linux `CLOCK_REALTIME_ALARM`, a Windows `fResume` and macOS power events SHALL NOT be used.

#### Scenario: A time during a suspend
- **GIVEN** a timer waiting until ten seconds from now
- **WHEN** the machine is suspended for thirty seconds
- **THEN** the machine stays asleep for the thirty seconds

### Requirement: A failing OS call is an error
A timer whose OS resources cannot be taken, armed or waited on, and a waker whose OS object cannot be rung, SHALL answer `CeichasError`, naming the call and carrying the OS error, and SHALL NOT panic. A wait interrupted by a signal SHALL go on rather than answer an error.

## Check table

How each requirement is known to hold, per backend and OS. The columns stand for the requirements above: a wait ending at its time and a waker ending it, the wall clock followed, suspended time counting, and a sleeping machine never woken.

| backend | OS | time reached, waker | clock followed | suspended time counts | machine left asleep |
| --- | --- | --- | --- | --- | --- |
| `Timerfd` | Linux | CI, VM | CI, VM | VM | VM |
| `Timerfd` | Android | expected | expected | expected | expected |
| `WaitableTimer` | Windows | CI | CI | expected | expected |
| `Dispatch` | macOS | CI | CI | expected | expected |
| `Polling` | Linux | CI, VM | CI, VM | VM | VM |
| `Polling` | macOS, Windows | CI | CI | expected | expected |
| `Polling` | Android, iOS, the BSDs and the rest | expected | expected | expected | expected |

- **CI**: the Tests workflow on GitHub's ubuntu-latest, macos-latest and windows-latest runners, on every push: tests/timer.rs, and tests/clock.rs in the clock job, time synchronisation stopped first. Android, iOS and FreeBSD get `cargo clippy --target` only, no run.
- **VM**: nix/vm-test.nix, the flake's `checks.x86_64-linux.vm`, in CI and locally with KVM: a NixOS guest waits until a datetime, has its clock set forward and back as root, and suspends to RAM for thirty seconds over a time ten seconds ahead, a monotonic `sleep` beside the wait as control.
- **expected**: from the OS source or documentation alone, no check yet:
  - `Timerfd` on Android: Android runs the Linux kernel's `timerfd`.
  - `WaitableTimer`, suspend: an absolute due time follows the system time, which Windows documents for clock changes ("If the system time is adjusted, the due time of any outstanding absolute timers is adjusted") but not for a resume.
  - `WaitableTimer`, machine left asleep: `fResume` is false, and "Otherwise, the system is not restored".
  - `Dispatch`, suspend: libdispatch arms wall timers with `NOTE_MACH_CONTINUOUS_TIME`, a clock that ticks across sleep (src/event/event_kevent.c, since libdispatch-913, macOS 10.13).
  - `Dispatch`, machine left asleep: a dispatch timer schedules no power event.
  - `Polling`: a step lasts at most one second while the process runs, and the wall clock is read after each; a condition variable wakes no machine.
