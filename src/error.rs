//! # Error
//!
//! [`CeichasError`], the error of every fallible call of a
//! [`CeichasTimer`](crate::timer::CeichasTimer) and its
//! [`CeichasWaker`](crate::waker::CeichasWaker).

use std::{error::Error, fmt, io};

/// An OS call of a timer or a waker failed.
#[derive(Debug)]
pub struct CeichasError {
    /// The OS call that failed.
    pub call: &'static str,
    /// The error the OS answered.
    pub source: io::Error,
}

impl fmt::Display for CeichasError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "OS call {} failed", self.call)
    }
}

impl Error for CeichasError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.source)
    }
}
