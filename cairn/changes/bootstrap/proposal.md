---
cairn: change
id: bootstrap
status: active
created: 2026-11-05
---

# Wait until a wall-clock time

The name is сейчас, "now": the crate says when now has come.

## Why

carillon's alarm hooks (carillon change alarm-hooks) must run at a given time: an event's alarm at 08:45, whatever happened before it. Rust's std cannot ask for that. `thread::sleep`, `Condvar::wait_timeout` and every async runtime timer (tokio, async-io, calloop) measure a duration on a clock that stops while the machine sleeps (Linux `CLOCK_MONOTONIC`, the macOS uptime clock; Windows 8 and later leave low-power time out of wait timeouts). Sleeping `08:45 - now` fires late by the length of a suspend, and misses a clock set by NTP or by hand. No crate fills the gap on all three platforms: `systime` (0.0.3) times durations counting sleep but follows no wall clock and marks Windows incomplete, `timerfd` and `nix::sys::timerfd` are Linux only, `boot-time` reads a clock and waits on none, and the other candidates need tokio (tokio-osinterval, tokio-timerd), are chrono-based queues (wallclock-timer, timer-deque-rs) or are a CLI (asleep).

Each OS has the timer, unprivileged: Linux `timerfd` on `CLOCK_REALTIME` with an absolute time, a macOS dispatch timer on `dispatch_walltime`, a Windows waitable timer given an absolute UTC due time. ceichas puts one interface over the three, so a caller runs its hook at the time it means.

## What

### The interface

A blocking primitive over the OS timer, everything else left to the caller:

- `CeichasTimer::new(CeichasTimerOptions)` takes the OS resources of one timer on the backend the options name, `CeichasTimerOptions::default()` naming the target's default, and `CeichasTimer::backend()` tells which. The options are a struct with pub fields so the wake opt-in (Not done) lands as one more field.
- `CeichasTimer::wait_until(&mut self, SystemTime) -> Result<CeichasWait, CeichasError>` blocks until the wall clock reaches the time (`CeichasWait::Reached`) or a waker wakes it (`CeichasWait::Woken`). A time already past returns at once.
- `CeichasTimer::waker(&self) -> CeichasWaker`, `Clone + Send + Sync`, whose `wake()` ends the current wait from any thread. A wake sent while no one waits is kept for the next wait, several collapsing into one, so a caller computing its next time never loses the wake that moved it.

The hook is the caller's: it calls `wait_until` with its next time, runs what it wants on `Reached`, and recomputes on `Woken`. A queue of times, the hooks themselves and their threads stay out, as carillon's alarm queue stays in carillon.

A pending wake wins over a reached time: a wait finding both answers `Woken`. A wake means "what you wait for may have moved", so the caller recomputes before acting on a stale time, however its loop is written: a caller doing `let next = queue.first(); if wait_until(next.at)? == Reached { fire(next) }` would otherwise fire the 08:45 alarm of an event moved to 09:45 a moment earlier. A wake is consumed only by a wait answering `Woken`, so a wake landing while a wait returns `Reached` stays pending for the next one. Every backend builds this the same way: an atomic flag the wakers raise and each wait checks before the clock, the OS object (eventfd, event, semaphore, condvar) only ringing the waiting thread awake when the flag goes up.

A waker outliving its timer is harmless: the OS object it rings is shared, so a late wake never lands on a reused file descriptor. `wake` is not async-signal-safe: a signal handler hands off to a thread first.

### Guarantees

- Never early: a backend's signal is checked against `SystemTime::now()`, and the wait goes on if the time is not reached.
- At most one second late while the machine runs, on every backend.
- Suspended time counts: a time passing while the machine sleeps is `Reached` within one second of the process running again.
- The wall clock is followed: a clock set past the time ends the wait within one second, a clock set back keeps it going until the new wall time reaches it.
- No polling on the native backends: their thread wakes only when its time is reached or a waker rings it, the OS re-basing the time on clock changes.
- No wake of a sleeping machine: the timer waits for the machine, never rouses it.
- A time before the UNIX epoch is past; a time beyond what the OS timer holds (past 2116 on `Dispatch`, 2262 on `Timerfd`) is ended by a wake alone; a wait interrupted by a signal goes on.

### Backends

A backend is one way to wait, as a `Base64Backend` is one way to encode: `CeichasBackend`, an enum whose variants exist on the targets that have them, so a backend a target lacks cannot be named there. Every backend holds the same guarantees; what differs is the OS mechanism and its dependencies. `CeichasBackend::default()` is the target's native backend, else `Polling`, and `CeichasBackend::available()` lists every one compiled in, its default first, so a caller distrusting a native timer picks `Polling` and the tests run the suite against each.

| backend | targets | timer | wake | clock change | suspend |
| --- | --- | --- | --- | --- | --- |
| `Timerfd` | Linux, Android | `timerfd_create(CLOCK_REALTIME)` armed with `TFD_TIMER_ABSTIME` | `eventfd`, both under one `poll` | the kernel re-bases absolute `CLOCK_REALTIME` timers on every clock set (`clock_was_set`) | fires on resume (`hrtimers_resume_local`, s2idle included) |
| `WaitableTimer` | Windows | `CreateWaitableTimerExW`, `SetWaitableTimer` with a positive (absolute UTC) due time, `fResume` false | an auto-reset event, both under `WaitForMultipleObjects` | documented: "If the system time is adjusted, the due time of any outstanding absolute timers is adjusted" | undocumented: relative due times leave low-power time out since Windows 8, absolute ones are expected to fire on resume |
| `Dispatch` | macOS | a dispatch source timer started at `dispatch_walltime`, `DISPATCH_TIMER_STRICT` | a dispatch semaphore the source and the wakers signal | libdispatch re-programs wall timers on `HOST_NOTIFY_CALENDAR_SET`, renewed after each notification | libdispatch arms wall timers with `NOTE_MACH_CONTINUOUS_TIME`, which ticks across sleep |
| `Polling` | every target where std has threads | `Condvar::wait_timeout` in steps of at most one second, reading `SystemTime::now()` at each | the condvar | within one step | within one step |

Defaults: `Timerfd` on Linux and Android, `WaitableTimer` on Windows, `Dispatch` on macOS, `Polling` elsewhere (the BSDs, iOS and the rest). The spec carries a check table: per backend and OS, each guarantee checked in CI, checked by hand, or expected from the OS source alone. A native backend found failing a guarantee on an OS stops being the default there, `Polling` taking over; no backend carries a slower fallback mode of its own.

The macOS choice is settled from libdispatch's source (src/event/event_kevent.c, since libdispatch-913, macOS 10.13): wall timers are armed `NOTE_ABSOLUTE | NOTE_MACH_CONTINUOUS_TIME`, and arming one registers `HOST_NOTIFY_CALENDAR_SET`, whose handler marks the wall timers for re-programming. The checks confirm it.

Left out:

- `kqueue`. Its absolute timers turn the wall deadline into an uptime countdown once, when armed: xnu's `filt_timervalidate` says "the conversion through wall-time is only done once" for `NOTE_ABSOLUTE`, FreeBSD's computes `to - boottime` for `NOTE_ABSTIME` and waits on `sbinuptime`. They follow no clock change, and Darwin's stops during sleep without `NOTE_MACH_CONTINUOUS_TIME`. Making them correct means rebuilding libdispatch on Apple and re-reading the wall clock periodically on the BSDs, a worse `Polling`. The BSD path for later is FreeBSD 14's `timerfd`, which re-bases absolute `CLOCK_REALTIME` timers when the clock jumps (`tc_setclock` calls `timerfd_jumped`), resume included (`acpi_resync_clock`, `inittodr`).
- `TFD_TIMER_CANCEL_ON_SET`. It reports a clock jump by failing the next read with `ECANCELED`, for callers that recompute their target from the new time (local-time calendars). ceichas waits for a fixed instant the kernel already re-bases, and the flag would wake it on every clock set and every resume (`timekeeping_resume` calls `timerfd_resume`) to arm the same time again.

Dependencies, each on its targets only: rustix for `Timerfd` (`timerfd`, `eventfd`, `poll`), windows-sys for `WaitableTimer`, a few `extern "C"` libdispatch symbols from libSystem for `Dispatch`. No cargo feature per backend: a target compiles its own and `Polling`. `unsafe` stays in the `WaitableTimer` and `Dispatch` modules.

### Tests

The core is the timer itself: a datetime triggers at that datetime, never before it and at most one second after it.

Automated, against every backend `CeichasBackend::available()` lists, in CI on Linux, macOS and Windows, each wait behind a deadline so a timer that never fires fails instead of hanging: a datetime reached on time, a past time and a time before the UNIX epoch at once, a far-future time ended by a wake, a wake during a wait, a wake before the wait kept and two wakes collapsed, a pending wake winning over a reached time, a waker outliving its timer, a timer armed many times in a row.

Clock changes: tests setting the system clock forward past a time and back before one, ignored by default and refusing to run without `CEICHAS_CLOCK_TESTS=1`, the clock restored by a guard. A CI job runs them on the Linux, macOS and Windows runners (passwordless sudo, administrator), time synchronisation stopped first. They never run on a workstation.

Suspend, wherever one is available: a NixOS VM test (`pkgs.testers.runNixOSTest`) suspends a Linux guest over a time with `rtcwake`, a monotonic `sleep` beside it as the control proving the suspend stopped the monotonic clock, and also runs the datetime and the clock changes as root. It runs in CI and locally with KVM, through examples/at.rs, which waits until a UNIX time or an offset and prints when it answered. GitHub's macOS and Windows runners cannot suspend: suspend stays unchecked there, expected from the libdispatch source and the Windows timer model, until a check on hardware. Android gets one Termux run of the Linux checks.

## Not done

- Waking a suspended machine, opt-in, kept reachable as a field of `CeichasTimerOptions`, since on Linux the clock (`CLOCK_REALTIME_ALARM`) and the `CAP_WAKE_ALARM` check are fixed at `timerfd_create`: Windows `fResume` true (honoured when the power plan allows wake timers), macOS `IOPMSchedulePowerEvent` (root). It comes with a NixOS module granting `CAP_WAKE_ALARM` to a binary through `security.wrappers`.
- The BSDs on FreeBSD 14's `timerfd`, and iOS on `Dispatch`, once a consumer needs them.
- Async: the Linux fd and the Windows handles fit an async reactor; nothing is exposed for it yet.
- A CLI running a command at a time: a leaf, behind a default-off `cli` feature if a use appears; examples/at.rs serves the VM test meanwhile.
