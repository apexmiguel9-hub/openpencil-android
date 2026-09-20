//! Calendar arithmetic for the inbox envelope.
//!
//! The Hub pins `captured_at` to an RFC 3339 instant in **UTC** — a local
//! offset is refused outright (`op-hub/backend/internal/snapshots/decode.go`
//! `parseCaptureTime`), and the value is rejected when it disagrees with
//! server time by more than 24 hours. So the client has to format the instant
//! itself rather than hand over whatever `Date.prototype.toString` produced,
//! and formatting it here rather than in JavaScript is what makes the exact
//! bytes testable on the host target.
//!
//! Two formats come out of the same instant:
//!
//! * `2026-08-04T09:20:31Z` — the wire value.
//! * `2026-08-04 17:20` — the local-time stamp appended to the snapshot's
//!   display name, because a user reading their inbox reads it in the
//!   timezone they captured in. The offset is supplied by the caller
//!   (`-new Date().getTimezoneOffset()`); this module never asks what time it
//!   is, so every function here is a pure function of its arguments.
//!
//! No date crate is pulled in for this: the whole job is one well-known
//! civil-from-days conversion, and this crate's dependency list is deliberately
//! three entries long.

/// Seconds in a day.
const DAY: i64 = 86_400;

/// The widest instant JavaScript's `Date` can represent, in milliseconds.
/// A value beyond it did not come from a clock, so it is clamped rather than
/// wrapped into some other year.
const MAX_JS_DATE_MS: f64 = 8.64e15;

/// Whole seconds since the Unix epoch, from a JavaScript millisecond value.
///
/// Non-finite input becomes 0 — an instant the Hub refuses for skew, which is
/// the right outcome for a caller that lost track of the clock: a refused
/// upload with a message, not a snapshot filed under a fabricated date.
pub fn unix_seconds_from_ms(ms: f64) -> i64 {
    if !ms.is_finite() {
        return 0;
    }
    let clamped = ms.clamp(-MAX_JS_DATE_MS, MAX_JS_DATE_MS);
    // Floor, not truncate: milliseconds before the epoch must not round up
    // into the following second.
    (clamped / 1000.0).floor() as i64
}

/// `2026-08-04T09:20:31Z` — RFC 3339, UTC, second precision.
pub fn format_rfc3339_utc(unix_seconds: i64) -> String {
    let (year, month, day, hour, minute, second) = split(unix_seconds);
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
}

/// `2026-08-04 17:20` — the same instant in the caller's timezone, to the
/// minute. Used only inside the display name.
pub fn format_local_stamp(unix_seconds: i64, offset_minutes: i32) -> String {
    // A real offset is within ±14 h; anything else is a caller that lost its
    // way, and shifting the stamp by a fabricated amount would be worse than
    // ignoring it.
    let offset = if (-14 * 60..=14 * 60).contains(&offset_minutes) {
        i64::from(offset_minutes)
    } else {
        0
    };
    let (year, month, day, hour, minute, _) = split(unix_seconds + offset * 60);
    format!("{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}")
}

/// Split an epoch second into `(year, month, day, hour, minute, second)`.
fn split(unix_seconds: i64) -> (i64, i64, i64, i64, i64, i64) {
    // `div_euclid` / `rem_euclid` rather than `/` and `%`: the remainder must
    // stay non-negative for instants before 1970, or the clock reads backwards
    // on the last day of 1969.
    let days = unix_seconds.div_euclid(DAY);
    let rest = unix_seconds.rem_euclid(DAY);
    let (year, month, day) = civil_from_days(days);
    (year, month, day, rest / 3600, (rest / 60) % 60, rest % 60)
}

/// Days since 1970-01-01 → proleptic Gregorian `(year, month, day)`.
///
/// Howard Hinnant's `civil_from_days`, the same algorithm the C++ standard's
/// chrono calendar is specified with. It is exact for the whole `i64` range
/// this crate can produce and has no branches on leap years.
fn civil_from_days(days: i64) -> (i64, i64, i64) {
    // Shift the epoch to 0000-03-01 so a leap day lands at the end of a cycle.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let day_of_era = z.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted_month = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * shifted_month + 2) / 5 + 1;
    let month = if shifted_month < 10 {
        shifted_month + 3
    } else {
        shifted_month - 9
    };
    (if month <= 2 { year + 1 } else { year }, month, day)
}
