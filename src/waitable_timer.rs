//! # Waitable timer
//!
//! The Windows backend: a waitable timer given an absolute UTC due time,
//! whose due time the system adjusts when its time is set, and an
//! auto-reset event the wakers set, both under one `WaitForMultipleObjects`.

use std::{
    io,
    os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle},
    ptr,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{SystemTime, UNIX_EPOCH},
};

use windows_sys::Win32::{
    Foundation::{FALSE, WAIT_FAILED},
    System::Threading::{
        CreateEventW, CreateWaitableTimerExW, INFINITE, SetEvent, SetWaitableTimer,
        TIMER_ALL_ACCESS, WaitForMultipleObjects,
    },
};

use crate::{error::CeichasError, timer::CeichasWait};

/// The 100-nanosecond intervals from 1601-01-01 to 1970-01-01, both UTC.
const UNIX_EPOCH_INTERVALS: i64 = 116_444_736_000_000_000;

#[derive(Debug)]
pub(crate) struct WaitableTimer {
    timer: OwnedHandle,
    shared: Arc<WaitableTimerShared>,
}

impl WaitableTimer {
    pub(crate) fn new() -> Result<Self, CeichasError> {
        // SAFETY: null attributes and name are valid, and the flags zero
        // asks for an auto-reset timer.
        let timer =
            unsafe { CreateWaitableTimerExW(ptr::null(), ptr::null(), 0, TIMER_ALL_ACCESS) };

        if timer.is_null() {
            return Err(CeichasError {
                call: "CreateWaitableTimerExW",
                source: io::Error::last_os_error(),
            });
        }

        // SAFETY: the handle is open, checked above, and owned by nothing
        // else.
        let timer = unsafe { OwnedHandle::from_raw_handle(timer) };

        // SAFETY: null attributes and name are valid; an auto-reset event,
        // initially unset.
        let bell = unsafe { CreateEventW(ptr::null(), FALSE, FALSE, ptr::null()) };

        if bell.is_null() {
            return Err(CeichasError {
                call: "CreateEventW",
                source: io::Error::last_os_error(),
            });
        }

        // SAFETY: the handle is open, checked above, and owned by nothing
        // else.
        let bell = unsafe { OwnedHandle::from_raw_handle(bell) };

        let shared = Arc::new(WaitableTimerShared {
            woken: AtomicBool::new(false),
            bell,
        });

        Ok(Self { timer, shared })
    }

    pub(crate) fn waker(&self) -> WaitableTimerWaker {
        WaitableTimerWaker {
            shared: self.shared.clone(),
        }
    }

    pub(crate) fn wait_until(&mut self, at: SystemTime) -> Result<CeichasWait, CeichasError> {
        loop {
            if self.shared.woken.swap(false, Ordering::AcqRel) {
                return Ok(CeichasWait::Woken);
            }

            if SystemTime::now() >= at {
                return Ok(CeichasWait::Reached);
            }

            let due = due_time(at);

            // SAFETY: the timer handle is open and `due` outlives the call;
            // no period, no completion routine, and `fResume` false never
            // wakes a sleeping machine.
            let set = unsafe {
                SetWaitableTimer(
                    self.timer.as_raw_handle(),
                    &due,
                    0,
                    None,
                    ptr::null(),
                    FALSE,
                )
            };

            if set == 0 {
                return Err(CeichasError {
                    call: "SetWaitableTimer",
                    source: io::Error::last_os_error(),
                });
            }

            let handles = [self.shared.bell.as_raw_handle(), self.timer.as_raw_handle()];

            // SAFETY: both handles are open, and the array outlives the call.
            let wait = unsafe { WaitForMultipleObjects(2, handles.as_ptr(), FALSE, INFINITE) };

            if wait == WAIT_FAILED {
                return Err(CeichasError {
                    call: "WaitForMultipleObjects",
                    source: io::Error::last_os_error(),
                });
            }
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct WaitableTimerWaker {
    shared: Arc<WaitableTimerShared>,
}

impl WaitableTimerWaker {
    pub(crate) fn wake(&self) -> Result<(), CeichasError> {
        if self.shared.woken.swap(true, Ordering::AcqRel) {
            return Ok(());
        }

        // SAFETY: the event handle is open as long as `shared` lives.
        let set = unsafe { SetEvent(self.shared.bell.as_raw_handle()) };

        if set == 0 {
            return Err(CeichasError {
                call: "SetEvent",
                source: io::Error::last_os_error(),
            });
        }

        Ok(())
    }
}

/// The state a timer shares with its wakers.
#[derive(Debug)]
struct WaitableTimerShared {
    /// Whether a wake is pending.
    woken: AtomicBool,
    /// The event a waker sets when it raises `woken`.
    bell: OwnedHandle,
}

/// The absolute due time of `at`, in 100-nanosecond intervals since
/// 1601-01-01 UTC, kept positive since a negative one is relative.
fn due_time(at: SystemTime) -> i64 {
    let intervals = match at.duration_since(UNIX_EPOCH) {
        Ok(since) => i64::try_from(since.as_nanos() / 100)
            .unwrap_or(i64::MAX)
            .saturating_add(UNIX_EPOCH_INTERVALS),
        Err(err) => UNIX_EPOCH_INTERVALS
            .saturating_sub(i64::try_from(err.duration().as_nanos() / 100).unwrap_or(i64::MAX)),
    };

    intervals.max(1)
}
