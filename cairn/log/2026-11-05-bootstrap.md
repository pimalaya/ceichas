---
cairn: log
change: bootstrap
landed: 2026-11-05
---

# Wait until a wall-clock time

carillon's alarm hooks must run at a wall-clock time, and neither std nor a maintained crate waits until one on Linux, macOS and Windows, counting a suspend and following clock changes. Reviewed with the user before the code: kqueue and `TFD_TIMER_CANCEL_ON_SET` left out on the evidence of the xnu, FreeBSD and Linux sources, no slower fallback mode in any backend, `Dispatch` the macOS default, a pending wake winning over a reached time, `CeichasTimerOptions` rather than a `with_backend` constructor, `Polling` steps of one second and a one-second lateness bound, each native backend the default on its OS behind a check table, suspend tested wherever one is available, and the wake opt-in left for later with a NixOS module.

What landed:

- `CeichasTimer` (`new`, `backend`, `waker`, `wait_until`), `CeichasTimerOptions`, `CeichasWait`, `CeichasWaker`, `CeichasBackend` (`default`, `available`) and `CeichasError`, naming the OS call that failed.
- Four backends: `Timerfd` (rustix `timerfd` on `CLOCK_REALTIME` armed with `TFD_TIMER_ABSTIME`, an `eventfd`, one `poll`) on Linux and Android, `WaitableTimer` (windows-sys, an absolute UTC due time, `fResume` false, an auto-reset event under `WaitForMultipleObjects`) on Windows, `Dispatch` (libdispatch symbols from libSystem, a source timer at `dispatch_walltime` with `DISPATCH_TIMER_STRICT`, a semaphore) on macOS, and `Polling` (a condition variable, steps of one second) wherever std has threads.
- One wake design on every backend: an atomic flag the wakers raise and each wait reads before the clock, the OS object rung only when the flag goes up and shared with the wakers, so a late wake never lands on a reused descriptor.
- tests/timer.rs, the suite against every available backend, each wait behind a deadline; tests/clock.rs, ignored and refusing to run without `CEICHAS_CLOCK_TESTS=1`; examples/at.rs; the NixOS VM test nix/vm-test.nix; the Tests and Audit workflows.

Checked, all green on 241848e: fmt, clippy and the suite on the ubuntu-latest, macos-latest and windows-latest runners; the clock job on all three; the MSRV 1.85 build; clippy for aarch64-linux-android, aarch64-apple-ios and x86_64-unknown-freebsd; cargo-deny; the VM test, in CI and locally. In two local VM runs, over a 30 s suspend to RAM with the time 10 s ahead, `Timerfd` answered as the guest resumed (its report 22 to 25 ms ahead of the script's resume stamp) and `Polling` 446 to 457 ms after it, each wait spanning about 31 s of wall time and at most 1 s of monotonic time, while the monotonic control `sleep 10` ended about 30 s late.

Traps met: the 9p share of the host's store hangs after a guest suspend, so the VM keeps its store on a disk image; QEMU never wakes a guest from suspend to idle, so the guest suspends to RAM with S3 enabled; `wait_for_console_text` reads one console line per second, so the test polls the whole console log; GitHub's Windows runners set their clock back through the Hyper-V time sync, so the clock job stops `vmictimesync` beside `w32time`.

carillon's alarm thread waits on a `CeichasTimer` since carillon f8bfed9, git-pinned to 661be05.

Left: the Android Termux run of the Linux checks and the release, both the user's; suspend on macOS and Windows, expected from the libdispatch source and the Windows timer model until a check on hardware; the wake opt-in with its NixOS module, the BSDs on FreeBSD 14's `timerfd`, iOS on `Dispatch`, async, a CLI.

Capabilities moved: timer (new).
