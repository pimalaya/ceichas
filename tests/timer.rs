//! The timer suite, run against every backend the target compiles. Each
//! wait runs behind a deadline, so a timer that never fires fails instead of
//! hanging.

use std::{
    panic,
    sync::mpsc::{self, RecvTimeoutError},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use ceichas::{
    backend::CeichasBackend,
    timer::{CeichasTimer, CeichasTimerOptions, CeichasWait},
    waker::CeichasWaker,
};

/// The lateness the spec allows while the machine runs.
const LATE: Duration = Duration::from_secs(1);

/// How long a test waits for an answer before failing.
const DEADLINE: Duration = Duration::from_secs(30);

const HOUR: Duration = Duration::from_secs(3600);

#[test]
fn datetime_ahead_returns_reached_on_time() {
    for &backend in CeichasBackend::available() {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
        let at = UNIX_EPOCH + Duration::from_secs(now.as_secs() + 2);

        let (wait, answered) = within(move || {
            let wait = timer(backend).wait_until(at).unwrap();
            (wait, SystemTime::now())
        });

        assert_eq!(wait, CeichasWait::Reached, "{backend:?}");
        assert_on_time(backend, at, answered);
    }
}

#[test]
fn past_time_returns_reached_at_once() {
    for &backend in CeichasBackend::available() {
        let start = Instant::now();
        let at = SystemTime::now() - Duration::from_secs(1);
        let wait = within(move || timer(backend).wait_until(at).unwrap());

        assert_eq!(wait, CeichasWait::Reached, "{backend:?}");
        assert!(start.elapsed() <= LATE, "{backend:?} blocked");
    }
}

#[test]
fn time_before_epoch_returns_reached_at_once() {
    for &backend in CeichasBackend::available() {
        let start = Instant::now();
        let at = UNIX_EPOCH.checked_sub(Duration::from_secs(86_400)).unwrap();
        let wait = within(move || timer(backend).wait_until(at).unwrap());

        assert_eq!(wait, CeichasWait::Reached, "{backend:?}");
        assert!(start.elapsed() <= LATE, "{backend:?} blocked");
    }
}

#[test]
fn times_in_a_row_return_reached_on_time() {
    for &backend in CeichasBackend::available() {
        within(move || {
            let mut timer = timer(backend);

            for _ in 0..20 {
                let at = SystemTime::now() + Duration::from_millis(20);
                let wait = timer.wait_until(at).unwrap();

                assert_eq!(wait, CeichasWait::Reached, "{backend:?}");
                assert_on_time(backend, at, SystemTime::now());
            }
        });
    }
}

#[test]
fn wake_during_wait_returns_woken() {
    for &backend in CeichasBackend::available() {
        let mut timer = timer(backend);
        let at = SystemTime::now() + HOUR;
        wake_soon(timer.waker());

        let wait = within(move || timer.wait_until(at).unwrap());
        assert_eq!(wait, CeichasWait::Woken, "{backend:?}");
    }
}

#[test]
fn wakes_before_wait_end_that_wait_alone() {
    for &backend in CeichasBackend::available() {
        let mut timer = timer(backend);
        let waker = timer.waker();
        let at = SystemTime::now() + HOUR;

        waker.wake().unwrap();
        waker.wake().unwrap();

        let (timer, wait) = within(move || {
            let wait = timer.wait_until(at).unwrap();
            (timer, wait)
        });

        assert_eq!(wait, CeichasWait::Woken, "{backend:?}");

        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            let mut timer = timer;
            let _ = tx.send(timer.wait_until(at).unwrap());
        });

        let kept = rx.recv_timeout(Duration::from_millis(500));
        assert!(kept.is_err(), "{backend:?} kept a second wake");

        waker.wake().unwrap();
        let wait = rx.recv_timeout(DEADLINE).unwrap();
        assert_eq!(wait, CeichasWait::Woken, "{backend:?}");
    }
}

#[test]
fn wake_and_reached_time_return_woken_first() {
    for &backend in CeichasBackend::available() {
        let mut timer = timer(backend);
        let at = SystemTime::now() - Duration::from_secs(1);
        timer.waker().wake().unwrap();

        let waits = within(move || [timer.wait_until(at).unwrap(), timer.wait_until(at).unwrap()]);
        assert_eq!(
            waits,
            [CeichasWait::Woken, CeichasWait::Reached],
            "{backend:?}"
        );
    }
}

#[test]
fn far_future_time_returns_woken_on_wake() {
    for &backend in CeichasBackend::available() {
        let mut timer = timer(backend);
        let years = Duration::from_secs(10_000 * 365 * 86_400);
        let at = SystemTime::now().checked_add(years).unwrap();
        wake_soon(timer.waker());

        let wait = within(move || timer.wait_until(at).unwrap());
        assert_eq!(wait, CeichasWait::Woken, "{backend:?}");
    }
}

#[test]
fn waker_outliving_timer_returns_ok() {
    for &backend in CeichasBackend::available() {
        let timer = timer(backend);
        let waker = timer.waker();
        drop(timer);

        waker.wake().unwrap();
        waker.wake().unwrap();
    }
}

#[test]
fn default_options_take_native_backend() {
    let timer = CeichasTimer::new(CeichasTimerOptions::default()).unwrap();
    assert_eq!(timer.backend(), native());

    let available: &[_] = if native() == CeichasBackend::Polling {
        &[CeichasBackend::Polling]
    } else {
        &[native(), CeichasBackend::Polling]
    };

    assert_eq!(CeichasBackend::available(), available);
}

#[test]
fn timer_and_waker_cross_threads() {
    fn send<T: Send>() {}
    fn sync<T: Sync>() {}

    send::<CeichasTimer>();
    send::<CeichasWaker>();
    sync::<CeichasWaker>();
}

fn timer(backend: CeichasBackend) -> CeichasTimer {
    CeichasTimer::new(CeichasTimerOptions { backend }).unwrap()
}

/// Runs `f` on its own thread, failing when it has not answered within
/// [`DEADLINE`].
fn within<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> T {
    let (tx, rx) = mpsc::channel();
    let handle = thread::spawn(move || {
        let _ = tx.send(f());
    });

    match rx.recv_timeout(DEADLINE) {
        Ok(answer) => answer,
        Err(RecvTimeoutError::Disconnected) => panic::resume_unwind(handle.join().unwrap_err()),
        Err(RecvTimeoutError::Timeout) => panic!("no answer within {DEADLINE:?}"),
    }
}

/// Wakes `waker` from another thread a moment from now.
fn wake_soon(waker: CeichasWaker) {
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(100));
        waker.wake().unwrap();
    });
}

fn assert_on_time(backend: CeichasBackend, at: SystemTime, answered: SystemTime) {
    let late = answered
        .duration_since(at)
        .unwrap_or_else(|_| panic!("{backend:?} answered before its time"));

    assert!(late <= LATE, "{backend:?} answered {late:?} late");
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn native() -> CeichasBackend {
    CeichasBackend::Timerfd
}

#[cfg(windows)]
fn native() -> CeichasBackend {
    CeichasBackend::WaitableTimer
}

#[cfg(target_os = "macos")]
fn native() -> CeichasBackend {
    CeichasBackend::Dispatch
}

#[cfg(not(any(
    target_os = "linux",
    target_os = "android",
    windows,
    target_os = "macos"
)))]
fn native() -> CeichasBackend {
    CeichasBackend::Polling
}
