//! Waits until a UNIX time in seconds, or `+N` seconds from now, then prints
//! how the wait ended and when, in milliseconds since the UNIX epoch for
//! `answered_ms` and `target_ms`, and as elapsed wall and monotonic time for
//! `wall_ms` and `mono_ms`. A second argument `polling` forces that backend.
//!
//! ```sh
//! cargo run --example at -- +30
//! cargo run --example at -- 1793900000 polling
//! ```
//!
//! The NixOS VM test runs it around clock changes and suspends.

use std::{
    env,
    error::Error,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use ceichas::{
    backend::CeichasBackend,
    timer::{CeichasTimer, CeichasTimerOptions},
};

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = env::args().skip(1);
    let time = args
        .next()
        .ok_or("Missing time: a UNIX time in seconds, or +N seconds from now")?;

    let backend = match args.next().as_deref() {
        None => CeichasBackend::default(),
        Some("polling") => CeichasBackend::Polling,
        Some(other) => return Err(format!("Unknown backend {other}").into()),
    };

    let at = match time.strip_prefix('+') {
        Some(secs) => SystemTime::now() + Duration::from_secs(secs.parse()?),
        None => UNIX_EPOCH + Duration::from_secs(time.parse()?),
    };

    let mut timer = CeichasTimer::new(CeichasTimerOptions { backend })?;
    let wall = SystemTime::now();
    let mono = Instant::now();

    let wait = timer.wait_until(at)?;
    let answered = SystemTime::now();

    println!(
        "{wait:?} backend={:?} answered_ms={} target_ms={} wall_ms={} mono_ms={}",
        timer.backend(),
        millis(answered),
        millis(at),
        millis(answered) - millis(wall),
        mono.elapsed().as_millis(),
    );

    Ok(())
}

/// Milliseconds since the UNIX epoch, negative before it.
fn millis(time: SystemTime) -> i128 {
    match time.duration_since(UNIX_EPOCH) {
        Ok(since) => since.as_millis() as i128,
        Err(err) => -(err.duration().as_millis() as i128),
    }
}
