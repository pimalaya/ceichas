//! # Dispatch
//!
//! The macOS backend: a dispatch source timer started at a
//! `dispatch_walltime`, which libdispatch arms on the continuous clock
//! (counting sleep) and re-programs on every calendar change, and a
//! semaphore the timer and the wakers signal.

use std::{
    ffi::c_void,
    io,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use crate::{error::CeichasError, timer::CeichasWait};

/// No time, as a timer start, a timer interval or a semaphore timeout.
const DISPATCH_TIME_FOREVER: u64 = !0;

/// The latest wall time libdispatch holds, in nanoseconds since the epoch.
const DISPATCH_WALLTIME_MAX: u128 = (1 << 62) - 1;

/// Asks a source timer to keep its leeway strictly.
const DISPATCH_TIMER_STRICT: usize = 0x1;

/// The global queue of default priority.
const DISPATCH_QUEUE_PRIORITY_DEFAULT: isize = 0;

#[derive(Debug)]
pub(crate) struct DispatchTimer {
    source: Source,
    shared: Arc<DispatchShared>,
}

impl DispatchTimer {
    pub(crate) fn new() -> Result<Self, CeichasError> {
        // SAFETY: a zero count is valid.
        let semaphore = unsafe { dispatch_semaphore_create(0) };

        if semaphore.is_null() {
            return Err(CeichasError {
                call: "dispatch_semaphore_create",
                source: io::Error::other("returned null"),
            });
        }

        let semaphore = Semaphore(semaphore);

        // SAFETY: the global queue of default priority always exists, and a
        // timer source takes no handle and the strict flag as mask.
        let source = unsafe {
            let queue = dispatch_get_global_queue(DISPATCH_QUEUE_PRIORITY_DEFAULT, 0);
            let kind = &raw const DISPATCH_SOURCE_TYPE_TIMER;
            dispatch_source_create(kind, 0, DISPATCH_TIMER_STRICT, queue)
        };

        if source.is_null() {
            return Err(CeichasError {
                call: "dispatch_source_create",
                source: io::Error::other("returned null"),
            });
        }

        let source = Source(source);

        // SAFETY: the source takes its own semaphore reference as context,
        // which its event handler signals and its cancel handler releases
        // once no event handler runs any more; a source starts suspended.
        unsafe {
            dispatch_retain(semaphore.0);
            dispatch_set_context(source.0, semaphore.0);
            dispatch_source_set_event_handler_f(source.0, signal);
            dispatch_source_set_cancel_handler_f(source.0, release);
            dispatch_resume(source.0);
        }

        let shared = Arc::new(DispatchShared {
            woken: AtomicBool::new(false),
            semaphore,
        });

        Ok(Self { source, shared })
    }

    pub(crate) fn waker(&self) -> DispatchWaker {
        DispatchWaker {
            shared: self.shared.clone(),
        }
    }

    pub(crate) fn wait_until(&mut self, at: SystemTime) -> CeichasWait {
        loop {
            if self.shared.woken.swap(false, Ordering::AcqRel) {
                return CeichasWait::Woken;
            }

            if SystemTime::now() >= at {
                return CeichasWait::Reached;
            }

            let start = walltime(at);

            // SAFETY: the source lives as long as the timer; a forever
            // interval fires once, and the zero leeway is kept strictly.
            unsafe { dispatch_source_set_timer(self.source.0, start, DISPATCH_TIME_FOREVER, 0) };

            // SAFETY: the semaphore lives as long as `shared`.
            unsafe { dispatch_semaphore_wait(self.shared.semaphore.0, DISPATCH_TIME_FOREVER) };
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct DispatchWaker {
    shared: Arc<DispatchShared>,
}

impl DispatchWaker {
    pub(crate) fn wake(&self) {
        if self.shared.woken.swap(true, Ordering::AcqRel) {
            return;
        }

        // SAFETY: the semaphore lives as long as `shared`.
        unsafe { dispatch_semaphore_signal(self.shared.semaphore.0) };
    }
}

/// The state a timer shares with its wakers.
#[derive(Debug)]
struct DispatchShared {
    /// Whether a wake is pending.
    woken: AtomicBool,
    /// The semaphore a waker signals when it raises `woken`.
    semaphore: Semaphore,
}

/// A dispatch semaphore reference, released on drop.
#[derive(Debug)]
struct Semaphore(*mut c_void);

// SAFETY: dispatch objects are reference counted and thread-safe, and a
// semaphore is signalled and waited on from any thread.
unsafe impl Send for Semaphore {}

// SAFETY: as for `Send`.
unsafe impl Sync for Semaphore {}

impl Drop for Semaphore {
    fn drop(&mut self) {
        // SAFETY: the reference is owned, and released once.
        unsafe { dispatch_release(self.0) };
    }
}

/// A dispatch source timer reference, cancelled and released on drop.
#[derive(Debug)]
struct Source(*mut c_void);

// SAFETY: dispatch objects are thread-safe, and a timer arms its source
// from one thread at once.
unsafe impl Send for Source {}

impl Drop for Source {
    fn drop(&mut self) {
        // SAFETY: the reference is owned, and released once; the cancel
        // runs the cancel handler, which releases the source's semaphore.
        unsafe {
            dispatch_source_cancel(self.0);
            dispatch_release(self.0);
        }
    }
}

/// Signals the semaphore a source timer holds as context.
extern "C" fn signal(semaphore: *mut c_void) {
    // SAFETY: the context is the source's own semaphore reference, released
    // by `release` only once no event handler runs any more.
    unsafe { dispatch_semaphore_signal(semaphore) };
}

/// Releases the semaphore a cancelled source timer held as context.
extern "C" fn release(semaphore: *mut c_void) {
    // SAFETY: libdispatch runs the cancel handler once, after the last
    // event handler.
    unsafe { dispatch_release(semaphore) };
}

/// The dispatch wall time of `at`, or no time beyond what libdispatch holds.
fn walltime(at: SystemTime) -> u64 {
    // NOTE: libdispatch reads one nanosecond or less as no time.
    let since = at
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .max(Duration::from_nanos(2));

    if since.as_nanos() > DISPATCH_WALLTIME_MAX {
        return DISPATCH_TIME_FOREVER;
    }

    let when = Timespec {
        tv_sec: since.as_secs() as i64,
        tv_nsec: i64::from(since.subsec_nanos()),
    };

    // SAFETY: `when` outlives the call.
    unsafe { dispatch_walltime(&when, 0) }
}

/// The C `struct timespec` of 64-bit Apple targets.
#[repr(C)]
struct Timespec {
    tv_sec: i64,
    tv_nsec: i64,
}

/// The opaque type of `DISPATCH_SOURCE_TYPE_TIMER`.
#[repr(C)]
struct DispatchSourceType {
    _private: [u8; 0],
}

// NOTE: libdispatch is part of libSystem, which every macOS binary links.
unsafe extern "C" {
    #[link_name = "_dispatch_source_type_timer"]
    static DISPATCH_SOURCE_TYPE_TIMER: DispatchSourceType;

    fn dispatch_get_global_queue(identifier: isize, flags: usize) -> *mut c_void;
    fn dispatch_retain(object: *mut c_void);
    fn dispatch_release(object: *mut c_void);
    fn dispatch_resume(object: *mut c_void);
    fn dispatch_set_context(object: *mut c_void, context: *mut c_void);
    fn dispatch_semaphore_create(value: isize) -> *mut c_void;
    fn dispatch_semaphore_signal(semaphore: *mut c_void) -> isize;
    fn dispatch_semaphore_wait(semaphore: *mut c_void, timeout: u64) -> isize;
    fn dispatch_source_cancel(source: *mut c_void);
    fn dispatch_source_create(
        kind: *const DispatchSourceType,
        handle: usize,
        mask: usize,
        queue: *mut c_void,
    ) -> *mut c_void;
    fn dispatch_source_set_cancel_handler_f(
        source: *mut c_void,
        handler: extern "C" fn(*mut c_void),
    );
    fn dispatch_source_set_event_handler_f(
        source: *mut c_void,
        handler: extern "C" fn(*mut c_void),
    );
    fn dispatch_source_set_timer(source: *mut c_void, start: u64, interval: u64, leeway: u64);
    fn dispatch_walltime(when: *const Timespec, delta: i64) -> u64;
}
