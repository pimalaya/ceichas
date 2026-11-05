//! # Backend
//!
//! [`CeichasBackend`], the OS timers a
//! [`CeichasTimer`](crate::timer::CeichasTimer) can wait on, each variant
//! existing only on the targets that have it.

/// One way to wait for a wall-clock time.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CeichasBackend {
    /// A condition variable waited on in steps of at most one second, the
    /// wall clock read at each.
    Polling,
    /// A `timerfd` on `CLOCK_REALTIME` armed with an absolute time, and an
    /// `eventfd` for the wakes, under one `poll`.
    #[cfg(any(target_os = "linux", target_os = "android"))]
    Timerfd,
    /// A waitable timer given an absolute UTC due time, and an event for the
    /// wakes, under one `WaitForMultipleObjects`.
    #[cfg(windows)]
    WaitableTimer,
    /// A dispatch source timer started at a `dispatch_walltime`, and a
    /// semaphore the timer and the wakes signal.
    #[cfg(target_os = "macos")]
    Dispatch,
}

impl CeichasBackend {
    /// Every backend the target compiles, its default first.
    pub const fn available() -> &'static [Self] {
        &[
            #[cfg(any(target_os = "linux", target_os = "android"))]
            Self::Timerfd,
            #[cfg(windows)]
            Self::WaitableTimer,
            #[cfg(target_os = "macos")]
            Self::Dispatch,
            Self::Polling,
        ]
    }
}

impl Default for CeichasBackend {
    /// The target's native backend, else [`CeichasBackend::Polling`].
    fn default() -> Self {
        Self::available()[0]
    }
}
