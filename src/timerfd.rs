//! # Timerfd
//!
//! The Linux and Android backend: a `timerfd` on `CLOCK_REALTIME` armed with
//! an absolute time, which the kernel re-bases on every clock set and fires
//! on resume, and an `eventfd` the wakers ring, both under one `poll`.

use std::{
    os::fd::OwnedFd,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use rustix::{
    event::{EventfdFlags, PollFd, PollFlags, eventfd, poll},
    io::{Errno, read, write},
    time::{
        Itimerspec, TimerfdClockId, TimerfdFlags, TimerfdTimerFlags, Timespec, timerfd_create,
        timerfd_settime,
    },
};

use crate::{error::CeichasError, timer::CeichasWait};

#[derive(Debug)]
pub(crate) struct TimerfdTimer {
    timer: OwnedFd,
    shared: Arc<TimerfdShared>,
}

impl TimerfdTimer {
    pub(crate) fn new() -> Result<Self, CeichasError> {
        let flags = TimerfdFlags::CLOEXEC | TimerfdFlags::NONBLOCK;
        let timer =
            timerfd_create(TimerfdClockId::Realtime, flags).map_err(|err| CeichasError {
                call: "timerfd_create",
                source: err.into(),
            })?;

        let flags = EventfdFlags::CLOEXEC | EventfdFlags::NONBLOCK;
        let bell = eventfd(0, flags).map_err(|err| CeichasError {
            call: "eventfd",
            source: err.into(),
        })?;

        let shared = Arc::new(TimerfdShared {
            woken: AtomicBool::new(false),
            bell,
        });

        Ok(Self { timer, shared })
    }

    pub(crate) fn waker(&self) -> TimerfdWaker {
        TimerfdWaker {
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

            // NOTE: a zero time disarms the timer.
            let since = at
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .max(Duration::from_nanos(1));

            let spec = Itimerspec {
                it_interval: Timespec {
                    tv_sec: 0,
                    tv_nsec: 0,
                },
                it_value: Timespec {
                    tv_sec: i64::try_from(since.as_secs()).unwrap_or(i64::MAX),
                    tv_nsec: since.subsec_nanos() as _,
                },
            };

            timerfd_settime(&self.timer, TimerfdTimerFlags::ABSTIME, &spec).map_err(|err| {
                CeichasError {
                    call: "timerfd_settime",
                    source: err.into(),
                }
            })?;

            let mut fds = [
                PollFd::new(&self.timer, PollFlags::IN),
                PollFd::new(&self.shared.bell, PollFlags::IN),
            ];

            match poll(&mut fds, None) {
                Ok(_) | Err(Errno::INTR) => (),
                Err(err) => {
                    return Err(CeichasError {
                        call: "poll",
                        source: err.into(),
                    });
                }
            }

            drain(&self.timer)?;
            drain(&self.shared.bell)?;
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct TimerfdWaker {
    shared: Arc<TimerfdShared>,
}

impl TimerfdWaker {
    pub(crate) fn wake(&self) -> Result<(), CeichasError> {
        if self.shared.woken.swap(true, Ordering::AcqRel) {
            return Ok(());
        }

        match write(&self.shared.bell, &1u64.to_ne_bytes()) {
            Ok(_) | Err(Errno::AGAIN) => Ok(()),
            Err(err) => Err(CeichasError {
                call: "write",
                source: err.into(),
            }),
        }
    }
}

/// The state a timer shares with its wakers.
#[derive(Debug)]
struct TimerfdShared {
    /// Whether a wake is pending.
    woken: AtomicBool,
    /// The `eventfd` a waker rings when it raises `woken`.
    bell: OwnedFd,
}

/// Empties the counter of a nonblocking `timerfd` or `eventfd`.
fn drain(fd: &OwnedFd) -> Result<(), CeichasError> {
    let mut count = [0; 8];

    match read(fd, &mut count) {
        Ok(_) | Err(Errno::AGAIN | Errno::INTR) => Ok(()),
        Err(err) => Err(CeichasError {
            call: "read",
            source: err.into(),
        }),
    }
}
