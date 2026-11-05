//! # Waker
//!
//! [`CeichasWaker`], ending the wait of a
//! [`CeichasTimer`](crate::timer::CeichasTimer) from any thread.

#[cfg(target_os = "macos")]
use crate::dispatch::DispatchWaker;
#[cfg(any(target_os = "linux", target_os = "android"))]
use crate::timerfd::TimerfdWaker;
#[cfg(windows)]
use crate::waitable_timer::WaitableTimerWaker;
use crate::{error::CeichasError, polling::PollingWaker};

/// Ends the current or the next wait of its timer, from any thread.
#[derive(Clone, Debug)]
pub struct CeichasWaker {
    pub(crate) waker: Waker,
}

impl CeichasWaker {
    /// Ends the wait in progress with
    /// [`CeichasWait::Woken`](crate::timer::CeichasWait::Woken), or the next
    /// one when none is in progress.
    ///
    /// Several wakes before one wait end that wait alone, and a waker
    /// outliving its timer wakes nothing. Not async-signal-safe.
    pub fn wake(&self) -> Result<(), CeichasError> {
        match &self.waker {
            Waker::Polling(waker) => {
                waker.wake();
                Ok(())
            }
            #[cfg(any(target_os = "linux", target_os = "android"))]
            Waker::Timerfd(waker) => waker.wake(),
            #[cfg(windows)]
            Waker::WaitableTimer(waker) => waker.wake(),
            #[cfg(target_os = "macos")]
            Waker::Dispatch(waker) => {
                waker.wake();
                Ok(())
            }
        }
    }
}

/// The OS side of a [`CeichasWaker`], one variant per backend.
#[derive(Clone, Debug)]
pub(crate) enum Waker {
    Polling(PollingWaker),
    #[cfg(any(target_os = "linux", target_os = "android"))]
    Timerfd(TimerfdWaker),
    #[cfg(windows)]
    WaitableTimer(WaitableTimerWaker),
    #[cfg(target_os = "macos")]
    Dispatch(DispatchWaker),
}
