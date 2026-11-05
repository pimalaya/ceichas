//! # ceichas CLI
//!
//! Waits until a wall-clock time on a [`CeichasTimer`], then exits 0: a
//! `sleep` that counts suspended time and follows clock changes, for shell
//! use (`ceichas 2026-11-06T08:45:00+01:00 && mpv alarm.ogg`). It runs no
//! command and installs no signal handler: a signal ends it the usual way.
//! Every output goes to stdout, errors included, and an error exits 1.

use std::{
    env,
    ops::Range,
    process::ExitCode,
    str,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use ceichas::{
    backend::CeichasBackend,
    timer::{CeichasTimer, CeichasTimerOptions, CeichasWait},
};

const HELP: &str = "\
Usage: ceichas [OPTIONS] <TIME>

Waits until the wall clock reaches TIME, then exits 0. Time spent suspended
counts, and clock changes are followed. An error exits 1.

TIME:
  2026-11-06T08:45:00+01:00  RFC 3339 date and time, Z or an offset, seconds
                             and fraction optional, T or a space between
  +90, +5m, +1h30m           duration from now, in s, m, h or d
  1793900000                 UNIX time in seconds

Options:
  -b, --backend <NAME>  OS timer to wait on (one of: BACKENDS)
  -v, --verbose         print the time before waiting and the wait after
  -h, --help            print this help
  -V, --version         print the version
";

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            println!("{err}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let mut backend = CeichasBackend::default();
    let mut verbose = false;
    let mut time = None;

    let mut args = env::args().skip(1);

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => {
                let names: Vec<_> = CeichasBackend::available()
                    .iter()
                    .map(|backend| backend_name(*backend))
                    .collect();
                print!("{}", HELP.replace("BACKENDS", &names.join(", ")));
                return Ok(());
            }
            "-V" | "--version" => {
                println!("ceichas {}", env!("CARGO_PKG_VERSION"));
                return Ok(());
            }
            "-v" | "--verbose" => verbose = true,
            "-b" | "--backend" => {
                let name = args.next().ok_or("Missing backend name after --backend")?;
                backend = CeichasBackend::available()
                    .iter()
                    .copied()
                    .find(|backend| backend_name(*backend) == name)
                    .ok_or(format!("Unknown backend {name} on this target"))?;
            }
            _ if time.is_none() => time = Some(arg),
            _ => return Err(format!("Unexpected argument {arg}")),
        }
    }

    let time = time.ok_or("Missing time, see --help")?;
    let at = parse_time(&time, SystemTime::now())?;

    let mut timer = CeichasTimer::new(CeichasTimerOptions { backend })
        .map_err(|err| format!("Cannot create timer: {err}: {}", err.source))?;

    let wall = SystemTime::now();
    let mono = Instant::now();

    if verbose {
        println!(
            "waiting backend={} target={} in_ms={}",
            backend_name(timer.backend()),
            format_time(at),
            millis(at) - millis(wall),
        );
    }

    let wait = timer
        .wait_until(at)
        .map_err(|err| format!("Cannot wait: {err}: {}", err.source))?;
    let answered = SystemTime::now();

    // NOTE: no waker exists, so no wait can be woken.
    if wait == CeichasWait::Woken {
        return Err("Cannot wait: woken without a waker".into());
    }

    if verbose {
        println!(
            "reached answered={} late_ms={} wall_ms={} mono_ms={}",
            format_time(answered),
            millis(answered) - millis(at),
            millis(answered) - millis(wall),
            mono.elapsed().as_millis(),
        );
    }

    Ok(())
}

/// The name `--backend` takes for a backend.
fn backend_name(backend: CeichasBackend) -> &'static str {
    match backend {
        CeichasBackend::Polling => "polling",
        #[cfg(any(target_os = "linux", target_os = "android"))]
        CeichasBackend::Timerfd => "timerfd",
        #[cfg(windows)]
        CeichasBackend::WaitableTimer => "waitable-timer",
        #[cfg(target_os = "macos")]
        CeichasBackend::Dispatch => "dispatch",
    }
}

/// Parses a TIME argument, an offset being taken from `now`.
fn parse_time(time: &str, now: SystemTime) -> Result<SystemTime, String> {
    if let Some(offset) = time.strip_prefix('+') {
        let offset = parse_offset(offset).ok_or(format!("Invalid duration {time}"))?;
        return now
            .checked_add(offset)
            .ok_or(format!("Duration {time} out of range"));
    }

    if !time.is_empty() && time.bytes().all(|b| b.is_ascii_digit()) {
        let secs = time
            .parse()
            .map_err(|_| format!("Invalid UNIX time {time}"))?;
        return UNIX_EPOCH
            .checked_add(Duration::from_secs(secs))
            .ok_or(format!("UNIX time {time} out of range"));
    }

    let (secs, nanos) = parse_rfc3339(time).ok_or(format!("Invalid time {time}, see --help"))?;

    let at = if secs >= 0 {
        UNIX_EPOCH.checked_add(Duration::new(secs.unsigned_abs(), nanos))
    } else {
        UNIX_EPOCH
            .checked_sub(Duration::from_secs(secs.unsigned_abs()))
            .and_then(|at| at.checked_add(Duration::from_nanos(nanos.into())))
    };

    at.ok_or(format!("Time {time} out of range"))
}

/// Parses `1h30m`, `5m` or `90`: numbers each followed by a unit among `s`,
/// `m`, `h` and `d`, the last one in seconds when it has none.
fn parse_offset(offset: &str) -> Option<Duration> {
    let mut total: u64 = 0;
    let mut number: Option<u64> = None;

    for c in offset.chars() {
        if let Some(digit) = c.to_digit(10) {
            number = Some(
                number
                    .unwrap_or(0)
                    .checked_mul(10)?
                    .checked_add(digit.into())?,
            );
            continue;
        }

        let unit = match c {
            's' => 1,
            'm' => 60,
            'h' => 3600,
            'd' => 86400,
            _ => return None,
        };

        total = total.checked_add(number.take()?.checked_mul(unit)?)?;
    }

    if offset.is_empty() {
        return None;
    }

    if let Some(secs) = number {
        total = total.checked_add(secs)?;
    }

    Some(Duration::from_secs(total))
}

/// Parses an RFC 3339 date and time into seconds since the UNIX epoch and
/// nanoseconds, the seconds and their fraction being optional and a space
/// allowed in place of the `T`. A leap second reads as the next second.
fn parse_rfc3339(time: &str) -> Option<(i64, u32)> {
    let bytes = time.as_bytes();

    let number = |range: Range<usize>| -> Option<i64> {
        let digits = bytes.get(range)?;

        if !digits.iter().all(u8::is_ascii_digit) {
            return None;
        }

        str::from_utf8(digits).ok()?.parse().ok()
    };

    let separator = |at: usize, allowed: &[u8]| bytes.get(at).is_some_and(|b| allowed.contains(b));

    let year = number(0..4)?;
    let month = number(5..7)?;
    let day = number(8..10)?;
    let hour = number(11..13)?;
    let minute = number(14..16)?;

    if !separator(4, b"-") || !separator(7, b"-") || !separator(10, b"Tt ") || !separator(13, b":")
    {
        return None;
    }

    let mut at = 16;
    let mut second = 0;
    let mut nanos = 0;

    if separator(at, b":") {
        second = number(at + 1..at + 3)?;
        at += 3;

        if separator(at, b".") {
            at += 1;
            let start = at;

            while bytes.get(at).is_some_and(u8::is_ascii_digit) {
                if at - start < 9 {
                    nanos = nanos * 10 + u32::from(bytes[at] - b'0');
                }
                at += 1;
            }

            if at == start {
                return None;
            }

            nanos *= 10u32.pow(9 - (at - start).min(9) as u32);
        }
    }

    let offset = match bytes.get(at)? {
        b'Z' | b'z' if at + 1 == bytes.len() => 0,
        sign @ (b'+' | b'-') if at + 6 == bytes.len() && separator(at + 3, b":") => {
            let hours = number(at + 1..at + 3)?;
            let minutes = number(at + 4..at + 6)?;

            if hours > 23 || minutes > 59 {
                return None;
            }

            let offset = hours * 3600 + minutes * 60;
            if *sign == b'-' { -offset } else { offset }
        }
        _ => return None,
    };

    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let days_in_month = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => return None,
    };

    if day < 1 || day > days_in_month || hour > 23 || minute > 59 || second > 60 {
        return None;
    }

    let days = days_from_civil(year, month, day);
    let secs = days * 86400 + hour * 3600 + minute * 60 + second - offset;

    Some((secs, nanos))
}

/// Days since 1970-01-01 of a proleptic Gregorian date (Howard Hinnant's
/// `days_from_civil`).
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let year_of_era = year - era * 400;
    let day_of_year = (153 * ((month + 9) % 12) + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;

    era * 146097 + day_of_era - 719468
}

/// Formats a time as an RFC 3339 UTC date and time to the millisecond
/// (Howard Hinnant's `civil_from_days`).
fn format_time(time: SystemTime) -> String {
    let millis = millis(time);
    let secs = millis.div_euclid(1000) as i64;
    let days = secs.div_euclid(86400);
    let second_of_day = secs.rem_euclid(86400);

    let days = days + 719468;
    let era = days.div_euclid(146097);
    let day_of_era = days - era * 146097;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36524 - day_of_era / 146096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_index = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_index + 2) / 5 + 1;
    let month = if month_index < 10 {
        month_index + 3
    } else {
        month_index - 9
    };
    let year = year_of_era + era * 400 + i64::from(month <= 2);

    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}.{:03}Z",
        second_of_day / 3600,
        second_of_day / 60 % 60,
        second_of_day % 60,
        millis.rem_euclid(1000),
    )
}

/// Milliseconds since the UNIX epoch, negative before it.
fn millis(time: SystemTime) -> i128 {
    match time.duration_since(UNIX_EPOCH) {
        Ok(since) => since.as_millis() as i128,
        Err(err) => -(err.duration().as_millis() as i128),
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, UNIX_EPOCH};

    use crate::*;

    #[test]
    fn rfc3339_offsets_and_fractions() {
        // 2026-11-06T07:45:00Z
        let expected = 1_793_951_100;

        assert_eq!(parse_rfc3339("2026-11-06T07:45:00Z"), Some((expected, 0)));
        assert_eq!(parse_rfc3339("2026-11-06t07:45z"), Some((expected, 0)));
        assert_eq!(
            parse_rfc3339("2026-11-06 08:45:00+01:00"),
            Some((expected, 0))
        );
        assert_eq!(
            parse_rfc3339("2026-11-06T02:15:00-05:30"),
            Some((expected, 0))
        );
        assert_eq!(
            parse_rfc3339("2026-11-06T07:45:00.5Z"),
            Some((expected, 500_000_000))
        );
        assert_eq!(
            parse_rfc3339("2026-11-06T07:45:00.1234567891Z"),
            Some((expected, 123_456_789)),
        );
    }

    #[test]
    fn rfc3339_calendar_edges() {
        assert_eq!(parse_rfc3339("1970-01-01T00:00:00Z"), Some((0, 0)));
        assert_eq!(parse_rfc3339("1969-12-31T23:59:59Z"), Some((-1, 0)));
        assert_eq!(
            parse_rfc3339("2000-02-29T00:00:00Z"),
            Some((951_782_400, 0))
        );
        assert_eq!(
            parse_rfc3339("2016-12-31T23:59:60Z"),
            Some((1_483_228_800, 0))
        );
        assert_eq!(parse_rfc3339("2100-02-29T00:00:00Z"), None);
        assert_eq!(parse_rfc3339("2026-04-31T00:00:00Z"), None);
        assert_eq!(parse_rfc3339("2026-11-06T24:00:00Z"), None);
        assert_eq!(parse_rfc3339("2026-11-06T07:45:00"), None);
        assert_eq!(parse_rfc3339("2026-11-06T07:45:00.Z"), None);
        assert_eq!(parse_rfc3339("2026-11-06T07:45:00+0100"), None);
        assert_eq!(parse_rfc3339("2026-11-06T07:45:00Zjunk"), None);
    }

    #[test]
    fn offsets() {
        assert_eq!(parse_offset("90"), Some(Duration::from_secs(90)));
        assert_eq!(parse_offset("5m"), Some(Duration::from_secs(300)));
        assert_eq!(parse_offset("1h30m"), Some(Duration::from_secs(5400)));
        assert_eq!(parse_offset("1d2"), Some(Duration::from_secs(86402)));
        assert_eq!(parse_offset(""), None);
        assert_eq!(parse_offset("m"), None);
        assert_eq!(parse_offset("5mm"), None);
        assert_eq!(parse_offset("5w"), None);
        assert_eq!(parse_offset("99999999999999999999"), None);
    }

    #[test]
    fn time_forms() {
        let now = UNIX_EPOCH + Duration::from_secs(1000);

        assert_eq!(parse_time("+1m", now), Ok(now + Duration::from_secs(60)));
        assert_eq!(
            parse_time("42", now),
            Ok(UNIX_EPOCH + Duration::from_secs(42))
        );
        assert_eq!(
            parse_time("1969-12-31T23:59:59.25Z", now),
            Ok(UNIX_EPOCH - Duration::from_millis(750)),
        );
        assert!(parse_time("tomorrow", now).is_err());
    }

    #[test]
    fn formats_round_trip() {
        for time in [
            "1969-12-31T23:59:59.250Z",
            "1970-01-01T00:00:00.000Z",
            "2000-02-29T12:34:56.789Z",
            "2026-11-06T07:45:00.000Z",
            "9999-12-31T23:59:59.999Z",
        ] {
            let at = parse_time(time, UNIX_EPOCH).unwrap();
            assert_eq!(format_time(at), time);
        }
    }
}
