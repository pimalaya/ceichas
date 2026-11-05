//! Clock changes, against every backend the target compiles. These tests
//! set the system clock through `sudo date` or `Set-Date`: ignored by
//! default, they refuse to run without `CEICHAS_CLOCK_TESTS=1` and belong to
//! CI runners and VMs, one at a time:
//!
//! ```sh
//! CEICHAS_CLOCK_TESTS=1 cargo test --test clock -- --ignored --test-threads=1
//! ```

use std::{
    env,
    process::Command,
    sync::mpsc::{self, Receiver},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use ceichas::{
    backend::CeichasBackend,
    timer::{CeichasTimer, CeichasTimerOptions, CeichasWait},
};

/// The lateness the spec allows after a clock change.
const LATE: Duration = Duration::from_secs(1);

const HOUR: Duration = Duration::from_secs(3600);

#[test]
#[ignore = "sets the system clock"]
fn clock_set_past_time_returns_reached() {
    let clock = Clock::take();

    for &backend in CeichasBackend::available() {
        let answer = wait(backend, SystemTime::now() + HOUR);

        thread::sleep(Duration::from_millis(500));
        assert!(answer.try_recv().is_err(), "{backend:?} answered early");

        clock.set(clock.untouched() + 2 * HOUR);

        let wait = answer
            .recv_timeout(LATE)
            .unwrap_or_else(|_| panic!("{backend:?} missed the clock set past its time"));

        assert_eq!(wait, CeichasWait::Reached, "{backend:?}");

        clock.set(clock.untouched());
    }
}

#[test]
#[ignore = "sets the system clock"]
fn clock_set_back_keeps_wait_going() {
    let clock = Clock::take();

    for &backend in CeichasBackend::available() {
        let answer = wait(backend, SystemTime::now() + Duration::from_secs(5));
        clock.set(clock.untouched() - Duration::from_secs(60));

        thread::sleep(Duration::from_secs(8));
        assert!(answer.try_recv().is_err(), "{backend:?} kept the old clock");

        clock.set(clock.untouched());

        let wait = answer
            .recv_timeout(LATE)
            .unwrap_or_else(|_| panic!("{backend:?} missed the clock set past its time"));

        assert_eq!(wait, CeichasWait::Reached, "{backend:?}");
    }
}

/// Waits until `at` on `backend` from another thread.
fn wait(backend: CeichasBackend, at: SystemTime) -> Receiver<CeichasWait> {
    let (tx, rx) = mpsc::channel();

    thread::spawn(move || {
        let mut timer = CeichasTimer::new(CeichasTimerOptions { backend }).unwrap();
        let _ = tx.send(timer.wait_until(at).unwrap());
    });

    rx
}

/// The system clock, set back on drop to what it would read untouched.
struct Clock {
    wall: SystemTime,
    mono: Instant,
}

impl Clock {
    fn take() -> Self {
        assert!(
            env::var_os("CEICHAS_CLOCK_TESTS").is_some(),
            "Refusing to set the system clock without CEICHAS_CLOCK_TESTS=1"
        );

        Self {
            wall: SystemTime::now(),
            mono: Instant::now(),
        }
    }

    /// What the clock would read had no test set it.
    fn untouched(&self) -> SystemTime {
        self.wall + self.mono.elapsed()
    }

    fn set(&self, time: SystemTime) {
        let since = time.duration_since(UNIX_EPOCH).unwrap();

        let mut command = if cfg!(target_os = "macos") {
            let mut command = Command::new("sudo");
            command.args(["date", "-u", "-f", "%s", &since.as_secs().to_string()]);
            command
        } else if cfg!(windows) {
            let script = format!(
                "Set-Date -Date ([DateTimeOffset]::FromUnixTimeMilliseconds({}).LocalDateTime) | Out-Null",
                since.as_millis()
            );

            let mut command = Command::new("powershell");
            command.args(["-NoProfile", "-Command", &script]);
            command
        } else {
            let time = format!("@{}.{:09}", since.as_secs(), since.subsec_nanos());
            let mut command = Command::new("sudo");
            command.args(["date", "-u", "-s", &time]);
            command
        };

        let status = command.status().expect("cannot run the clock command");
        assert!(status.success(), "cannot set the system clock");
    }
}

impl Drop for Clock {
    fn drop(&mut self) {
        self.set(self.untouched());
    }
}
