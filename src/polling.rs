//! # Polling
//!
//! The backend of every target: a condition variable waited on in steps of
//! at most one second, the wall clock read at each, so a suspend or a clock
//! change is seen within one step.

use std::{
    sync::{Arc, Condvar, Mutex, PoisonError},
    time::{Duration, SystemTime},
};

use crate::timer::CeichasWait;

/// The longest wait between two reads of the wall clock.
const STEP: Duration = Duration::from_secs(1);

#[derive(Debug)]
pub(crate) struct PollingTimer {
    shared: Arc<PollingShared>,
}

impl PollingTimer {
    pub(crate) fn new() -> Self {
        Self {
            shared: Arc::default(),
        }
    }

    pub(crate) fn waker(&self) -> PollingWaker {
        PollingWaker {
            shared: self.shared.clone(),
        }
    }

    pub(crate) fn wait_until(&mut self, at: SystemTime) -> CeichasWait {
        let mut woken = self
            .shared
            .woken
            .lock()
            .unwrap_or_else(PoisonError::into_inner);

        loop {
            if *woken {
                *woken = false;
                return CeichasWait::Woken;
            }

            let left = match at.duration_since(SystemTime::now()) {
                Ok(left) if !left.is_zero() => left,
                _ => return CeichasWait::Reached,
            };

            woken = self
                .shared
                .bell
                .wait_timeout(woken, left.min(STEP))
                .unwrap_or_else(PoisonError::into_inner)
                .0;
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct PollingWaker {
    shared: Arc<PollingShared>,
}

impl PollingWaker {
    pub(crate) fn wake(&self) {
        *self
            .shared
            .woken
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = true;

        self.shared.bell.notify_one();
    }
}

/// The state a timer shares with its wakers.
#[derive(Debug, Default)]
struct PollingShared {
    /// Whether a wake is pending.
    woken: Mutex<bool>,
    /// What a wait sleeps on between two reads of the wall clock.
    bell: Condvar,
}
