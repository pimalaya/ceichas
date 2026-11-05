//! # Timer
//!
//! [`CeichasTimer`], one OS timer waiting until a wall-clock time, built
//! from [`CeichasTimerOptions`] and answering a [`CeichasWait`]. Its
//! [`CeichasWaker`] ends a wait from another thread.

use std::time::SystemTime;

#[cfg(target_os = "macos")]
use crate::dispatch::DispatchTimer;
#[cfg(any(target_os = "linux", target_os = "android"))]
use crate::timerfd::TimerfdTimer;
#[cfg(windows)]
use crate::waitable_timer::WaitableTimer;
use crate::{
    backend::CeichasBackend,
    error::CeichasError,
    polling::PollingTimer,
    waker::{CeichasWaker, Waker},
};

/// One OS timer waiting until a wall-clock time, one wait at once.
#[derive(Debug)]
pub struct CeichasTimer {
    timer: Timer,
}

impl CeichasTimer {
    /// Takes the OS resources of one timer on the backend `options` names.
    pub fn new(options: CeichasTimerOptions) -> Result<Self, CeichasError> {
        let timer = match options.backend {
            CeichasBackend::Polling => Timer::Polling(PollingTimer::new()),
            #[cfg(any(target_os = "linux", target_os = "android"))]
            CeichasBackend::Timerfd => Timer::Timerfd(TimerfdTimer::new()?),
            #[cfg(windows)]
            CeichasBackend::WaitableTimer => Timer::WaitableTimer(WaitableTimer::new()?),
            #[cfg(target_os = "macos")]
            CeichasBackend::Dispatch => Timer::Dispatch(DispatchTimer::new()?),
        };

        Ok(Self { timer })
    }

    /// The backend the timer waits on.
    pub fn backend(&self) -> CeichasBackend {
        match &self.timer {
            Timer::Polling(_) => CeichasBackend::Polling,
            #[cfg(any(target_os = "linux", target_os = "android"))]
            Timer::Timerfd(_) => CeichasBackend::Timerfd,
            #[cfg(windows)]
            Timer::WaitableTimer(_) => CeichasBackend::WaitableTimer,
            #[cfg(target_os = "macos")]
            Timer::Dispatch(_) => CeichasBackend::Dispatch,
        }
    }

    /// A waker ending this timer's waits from any thread.
    pub fn waker(&self) -> CeichasWaker {
        let waker = match &self.timer {
            Timer::Polling(timer) => Waker::Polling(timer.waker()),
            #[cfg(any(target_os = "linux", target_os = "android"))]
            Timer::Timerfd(timer) => Waker::Timerfd(timer.waker()),
            #[cfg(windows)]
            Timer::WaitableTimer(timer) => Waker::WaitableTimer(timer.waker()),
            #[cfg(target_os = "macos")]
            Timer::Dispatch(timer) => Waker::Dispatch(timer.waker()),
        };

        CeichasWaker { waker }
    }

    /// Blocks until the wall clock reaches `at`, or until a waker rings.
    ///
    /// A time at or before now answers [`CeichasWait::Reached`] at once, and
    /// a pending wake [`CeichasWait::Woken`] at once, even with `at` reached.
    pub fn wait_until(&mut self, at: SystemTime) -> Result<CeichasWait, CeichasError> {
        match &mut self.timer {
            Timer::Polling(timer) => Ok(timer.wait_until(at)),
            #[cfg(any(target_os = "linux", target_os = "android"))]
            Timer::Timerfd(timer) => timer.wait_until(at),
            #[cfg(windows)]
            Timer::WaitableTimer(timer) => timer.wait_until(at),
            #[cfg(target_os = "macos")]
            Timer::Dispatch(timer) => Ok(timer.wait_until(at)),
        }
    }
}

/// What a [`CeichasTimer`] is built with.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct CeichasTimerOptions {
    /// The backend the timer waits on, the target's default by default.
    pub backend: CeichasBackend,
}

/// How a wait ended.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CeichasWait {
    /// The wall clock reached the time.
    Reached,
    /// A waker rang, before the time or as it was reached.
    Woken,
}

/// The OS timer of a [`CeichasTimer`], one variant per backend.
#[derive(Debug)]
enum Timer {
    Polling(PollingTimer),
    #[cfg(any(target_os = "linux", target_os = "android"))]
    Timerfd(TimerfdTimer),
    #[cfg(windows)]
    WaitableTimer(WaitableTimer),
    #[cfg(target_os = "macos")]
    Dispatch(DispatchTimer),
}
