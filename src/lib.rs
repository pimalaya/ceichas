#![cfg_attr(docsrs, feature(doc_cfg))]

//! # ceichas
//!
//! Waits until a wall-clock time. A [`CeichasTimer`](timer::CeichasTimer)
//! blocks until [`SystemTime::now`](std::time::SystemTime::now) reaches the
//! time it is given, counting the time the machine spends suspended and
//! following clock changes, which neither
//! [`thread::sleep`](std::thread::sleep) nor a condition variable do: both
//! measure a duration on a clock that stops during a suspend and ignores
//! clock changes.
//!
//! A [`CeichasWaker`](waker::CeichasWaker) ends a wait from any thread. A
//! wake sent while no one waits is kept for the next wait, several
//! collapsing into one, and a pending wake wins over a reached time, so a
//! caller recomputing its next time on
//! [`CeichasWait::Woken`](timer::CeichasWait::Woken) never acts on a time a
//! wake moved. The caller keeps its own queue of times: a timer waits for
//! one time at once.
//!
//! A wait never answers [`CeichasWait::Reached`](timer::CeichasWait::Reached)
//! early. It answers at most one second late while the machine runs, and
//! within one second of the process running again after a suspend over the
//! time or of the clock being set past it. A clock set back keeps the wait
//! going. A timer never wakes a sleeping machine.
//!
//! Each [`CeichasBackend`](backend::CeichasBackend) is one OS timer, its
//! variant existing only on the targets that have it:
//!
//! | backend | targets | timer |
//! | --- | --- | --- |
//! | `Timerfd` | Linux, Android | `timerfd` on `CLOCK_REALTIME` armed with an absolute time |
//! | `WaitableTimer` | Windows | waitable timer given an absolute UTC due time |
//! | `Dispatch` | macOS | dispatch source timer started at a `dispatch_walltime` |
//! | `Polling` | every target | condition variable waited on in steps of at most one second |
//!
//! The OS re-bases the native timers on clock changes and fires them on
//! resume, so their thread wakes only at the time or on a wake.
//! [`CeichasBackend::default`](backend::CeichasBackend::default) is the
//! target's native backend, else `Polling`.
//!
//! ```
//! use std::time::{Duration, SystemTime};
//!
//! use ceichas::timer::{CeichasTimer, CeichasTimerOptions, CeichasWait};
//!
//! let mut timer = CeichasTimer::new(CeichasTimerOptions::default())?;
//! let waker = timer.waker();
//!
//! let at = SystemTime::now() + Duration::from_millis(100);
//! assert_eq!(timer.wait_until(at)?, CeichasWait::Reached);
//!
//! // NOTE: a pending wake wins over a reached time.
//! waker.wake()?;
//! assert_eq!(timer.wait_until(at)?, CeichasWait::Woken);
//! # Ok::<(), ceichas::error::CeichasError>(())
//! ```

pub mod backend;
#[cfg(target_os = "macos")]
mod dispatch;
pub mod error;
mod polling;
pub mod timer;
#[cfg(any(target_os = "linux", target_os = "android"))]
mod timerfd;
#[cfg(windows)]
mod waitable_timer;
pub mod waker;
