//! `_date` builtin module -- native primitives backing `libs/date.zu`.
//!
//! `libs/date.zu` implements essentially everything itself in pure Zuri --
//! civil calendar math, `Date.format()`/`parse()`/`parse_format()`, the
//! whole `Date` class -- see that file's own doc comments (`grep -n
//! '_date\.' libs/date.zu` turns up exactly three call sites). This module
//! is the native backing for those three: the system's current UTC clock
//! (`gmtime()`), the system's current LOCAL clock with real timezone/DST
//! information (`localtime()`), and the inverse of the latter (`mktime()`
//! -- "what UTC epoch second does this local broken-down time correspond
//! to"). Everything else in `date.zu` -- including `Date.to_time()`, the
//! Julian-date conversions, and the format/parse machinery -- needs no
//! native support at all.
//!
//! `gmtime()`/`localtime()` return a dict shaped exactly like the
//! module's own docblock example:
//!
//! ```text
//! {year: 2022, month: 3, day: 5, week_day: 6, year_day: 63, hour: 17,
//!  minute: 30, seconds: 55, microseconds: 620290, is_dst: false,
//!  zone: UTC, gmt_offset: 0}
//! ```
//!
//! `week_day` is 0=Sunday..6=Saturday (matching both POSIX `tm_wday` and
//! `date.zu`'s own `_weekdays_short`/`_weekdays_long` indexing) and
//! `year_day` is 0-indexed (Jan 1st is day 0), matching `tm_yday` --
//! confirmed against the docblock's own example (2022-03-05, a non-leap
//! year: Jan(31)+Feb(28)=59 days before March, +4 more days to the 5th =
//! 63).
//!
//! ## Platform support
//!
//! On Unix, all three natives are thin wrappers around the platform's own
//! `gmtime_r`/`localtime_r`/`mktime` -- which is what gives `localtime()`
//! correct real-world DST and zone-abbreviation behavior for free, via
//! the system's own tzdata, rather than this crate trying to vendor a
//! timezone database.
//!
//! On non-Unix, there is no portable way to reach a timezone database, so
//! `localtime()` degrades to UTC (same "degrade to UTC" fallback used for
//! `mktime()`'s otherwise-local interpretation of its input) and
//! `gmtime()`/that fallback path both go through a small hand-rolled
//! proleptic-Gregorian calendar (Howard Hinnant's well-known
//! `civil_from_days`/`days_from_civil` algorithm) instead of `libc`.

use crate::builtins::enforce::ArgType;
use crate::modules::{BuiltinModuleDef, native};
use crate::vm::object::ZuriContext;
use crate::vm::value::Value;
use crate::vm::vm::VM;
use crate::{enforce_arg_count, enforce_arg_type};
use std::time::{SystemTime, UNIX_EPOCH};

pub static MODULE: BuiltinModuleDef = BuiltinModuleDef {
  name: "_date",
  build,
};

fn build(vm: &mut VM) -> Vec<(&'static str, Value)> {
  vec![
    ("gmtime", native(vm, "gmtime", 0, false, gmtime_fn)),
    ("localtime", native(vm, "localtime", 0, false, localtime_fn)),
    ("mktime", native(vm, "mktime", 7, false, mktime_fn)),
  ]
}

// Portable proleptic-Gregorian civil calendar math -- the non-Unix
// fallback. Correct across `date.zu`'s own documented year range
// (1..9999) and well beyond it.

/// Days since 1970-01-01 for the given proleptic-Gregorian civil date.
#[allow(dead_code)]
fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
  let y = if m <= 2 { y - 1 } else { y };
  let era = if y >= 0 { y } else { y - 399 } / 400;
  let yoe = y - era * 400; // [0, 399]
  let mp = (if m > 2 { m - 3 } else { m + 9 }) as i64; // [0, 11]
  let doy = (153 * mp + 2) / 5 + d as i64 - 1; // [0, 365]
  let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy; // [0, 146096]
  era * 146097 + doe - 719468
}

/// Inverse of `days_from_civil`.
#[allow(dead_code)]
fn civil_from_days(z: i64) -> (i64, u32, u32) {
  let z = z + 719468;
  let era = if z >= 0 { z } else { z - 146096 } / 146097;
  let doe = z - era * 146097; // [0, 146096]
  let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365; // [0, 399]
  let y = yoe + era * 400;
  let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
  let mp = (5 * doy + 2) / 153; // [0, 11]
  let d = (doy - (153 * mp + 2) / 5 + 1) as u32; // [1, 31]
  let m = (if mp < 10 { mp + 3 } else { mp - 9 }) as u32; // [1, 12]
  (if m <= 2 { y + 1 } else { y }, m, d)
}

/// 0 = Sunday .. 6 = Saturday. 1970-01-01 (z=0) was a Thursday.
#[allow(dead_code)]
fn weekday_from_days(z: i64) -> i64 {
  (z.rem_euclid(7) + 4) % 7
}

// Shared broken-down-time representation and dict construction

struct BrokenDown {
  year: i64,
  month: u32,
  day: u32,
  hour: u32,
  minute: u32,
  seconds: u32,
  week_day: i64,
  year_day: i64,
  is_dst: bool,
  zone: String,
  gmt_offset: i64,
}

/// Current wall-clock time as (whole seconds since epoch, microseconds
/// within that second) -- a single `SystemTime` sample so both halves
/// agree, rather than reading the clock twice.
fn now_secs_and_micros() -> (i64, i64) {
  match SystemTime::now().duration_since(UNIX_EPOCH) {
    Ok(d) => (d.as_secs() as i64, (d.subsec_nanos() / 1000) as i64),
    Err(e) => {
      // Clock set before the Unix epoch -- fall back to the negative
      // offset `e` already carries rather than panicking. Deliberately
      // rare; there's no natural way to propagate an Err from a helper
      // used unconditionally by every call site below.
      (-(e.duration().as_secs() as i64), 0)
    },
  }
}

#[allow(dead_code)]
fn gmtime_portable(secs: i64) -> BrokenDown {
  let days = secs.div_euclid(86400);
  let secs_of_day = secs.rem_euclid(86400);
  let (year, month, day) = civil_from_days(days);
  BrokenDown {
    year,
    month,
    day,
    hour: (secs_of_day / 3600) as u32,
    minute: ((secs_of_day % 3600) / 60) as u32,
    seconds: (secs_of_day % 60) as u32,
    week_day: weekday_from_days(days),
    year_day: days - days_from_civil(year, 1, 1),
    is_dst: false,
    zone: "UTC".to_string(),
    gmt_offset: 0,
  }
}

fn broken_down_to_dict(ctx: &mut ZuriContext, bd: BrokenDown, microseconds: i64) -> Value {
  let k_year = ctx.heap().alloc_string("year");
  let k_month = ctx.heap().alloc_string("month");
  let k_day = ctx.heap().alloc_string("day");
  let k_week_day = ctx.heap().alloc_string("week_day");
  let k_year_day = ctx.heap().alloc_string("year_day");
  let k_hour = ctx.heap().alloc_string("hour");
  let k_minute = ctx.heap().alloc_string("minute");
  let k_seconds = ctx.heap().alloc_string("seconds");
  let k_microseconds = ctx.heap().alloc_string("microseconds");
  let k_is_dst = ctx.heap().alloc_string("is_dst");
  let k_zone = ctx.heap().alloc_string("zone");
  let k_gmt_offset = ctx.heap().alloc_string("gmt_offset");
  let zone_val = ctx.heap().alloc_string(bd.zone);

  ctx.heap().alloc_dict(vec![
    (k_year, Value::number(bd.year as f64)),
    (k_month, Value::number(bd.month as f64)),
    (k_day, Value::number(bd.day as f64)),
    (k_week_day, Value::number(bd.week_day as f64)),
    (k_year_day, Value::number(bd.year_day as f64)),
    (k_hour, Value::number(bd.hour as f64)),
    (k_minute, Value::number(bd.minute as f64)),
    (k_seconds, Value::number(bd.seconds as f64)),
    (k_microseconds, Value::number(microseconds as f64)),
    (k_is_dst, Value::bool(bd.is_dst)),
    (k_zone, zone_val),
    (k_gmt_offset, Value::number(bd.gmt_offset as f64)),
  ])
}

// gmtime / localtime -- Unix

#[cfg(unix)]
fn tm_to_broken_down(tm: &libc::tm, is_utc: bool) -> BrokenDown {
  let zone = if is_utc {
    "UTC".to_string()
  } else if tm.tm_zone.is_null() {
    String::new()
  } else {
    // `tm_zone`/`tm_gmtoff` are glibc/BSD/macOS extensions (not strict
    // POSIX), but present on every Unix target this VM otherwise
    // already assumes elsewhere (see e.g. `os.rs`'s own `stats()`).
    unsafe { std::ffi::CStr::from_ptr(tm.tm_zone as *const _) }
      .to_string_lossy()
      .into_owned()
  };

  BrokenDown {
    year: tm.tm_year as i64 + 1900,
    month: tm.tm_mon as u32 + 1,
    day: tm.tm_mday as u32,
    hour: tm.tm_hour as u32,
    minute: tm.tm_min as u32,
    seconds: tm.tm_sec as u32,
    week_day: tm.tm_wday as i64,
    year_day: tm.tm_yday as i64,
    is_dst: tm.tm_isdst > 0,
    zone,
    gmt_offset: if is_utc { 0 } else { tm.tm_gmtoff as i64 },
  }
}

#[cfg(unix)]
fn gmtime_fn(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 0);
  let (secs, micros) = now_secs_and_micros();
  let time = secs as libc::time_t;
  let mut tm: libc::tm = unsafe { std::mem::zeroed() };
  if unsafe { libc::gmtime_r(&time, &mut tm) }.is_null() {
    return Err("gmtime(): the system clock produced an unrepresentable time".to_string());
  }
  let bd = tm_to_broken_down(&tm, true);
  Ok(broken_down_to_dict(ctx, bd, micros))
}

#[cfg(unix)]
fn localtime_fn(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 0);
  let (secs, micros) = now_secs_and_micros();
  let time = secs as libc::time_t;
  let mut tm: libc::tm = unsafe { std::mem::zeroed() };
  if unsafe { libc::localtime_r(&time, &mut tm) }.is_null() {
    return Err("localtime(): the system clock produced an unrepresentable time".to_string());
  }
  let bd = tm_to_broken_down(&tm, false);
  Ok(broken_down_to_dict(ctx, bd, micros))
}

// gmtime / localtime -- non-Unix fallback

#[cfg(not(unix))]
fn gmtime_fn(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 0);
  let (secs, micros) = now_secs_and_micros();
  let bd = gmtime_portable(secs);
  Ok(broken_down_to_dict(ctx, bd, micros))
}

#[cfg(not(unix))]
fn localtime_fn(ctx: &mut ZuriContext) -> Result<Value, String> {
  // No portable way to read the local timezone/DST rules without a
  // vendored tzdata -- degrade to UTC, same fallback `os.rs`'s own
  // `gather_uname` uses for platform facts it can't get portably either.
  gmtime_fn(ctx)
}

// mktime

fn mktime_fn(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 7);
  enforce_arg_type!(ctx, 0, ArgType::Number);
  enforce_arg_type!(ctx, 1, ArgType::Number);
  enforce_arg_type!(ctx, 2, ArgType::Number);
  enforce_arg_type!(ctx, 3, ArgType::Number);
  enforce_arg_type!(ctx, 4, ArgType::Number);
  enforce_arg_type!(ctx, 5, ArgType::Number);

  let year = ctx.args[0].as_number() as i64;
  let month = ctx.args[1].as_number() as i64;
  let day = ctx.args[2].as_number() as i64;
  let hour = ctx.args[3].as_number() as i64;
  let minute = ctx.args[4].as_number() as i64;
  let seconds = ctx.args[5].as_number() as i64;

  let is_dst = match ctx.args.get(6) {
    None => None,
    Some(v) if v.is_nil() => None,
    Some(v) if v.is_bool() => Some(v.as_bool()),
    Some(v) => {
      return Err(format!(
        "mktime(): is_dst must be a bool or nil, got {}",
        v.type_name()
      ));
    },
  };

  mktime_impl(year, month, day, hour, minute, seconds, is_dst)
}

#[cfg(unix)]
fn mktime_impl(
  year: i64,
  month: i64,
  day: i64,
  hour: i64,
  minute: i64,
  seconds: i64,
  is_dst: Option<bool>,
) -> Result<Value, String> {
  let mut tm: libc::tm = unsafe { std::mem::zeroed() };
  tm.tm_year = (year - 1900) as libc::c_int;
  tm.tm_mon = (month - 1) as libc::c_int;
  tm.tm_mday = day as libc::c_int;
  tm.tm_hour = hour as libc::c_int;
  tm.tm_min = minute as libc::c_int;
  tm.tm_sec = seconds as libc::c_int;
  // -1 asks libc to work out DST for itself from the given local time,
  // matching POSIX `mktime`'s own documented meaning for that value.
  tm.tm_isdst = match is_dst {
    None => -1,
    Some(true) => 1,
    Some(false) => 0,
  };

  let t = unsafe { libc::mktime(&mut tm) };
  if t == -1 {
    return Err("mktime(): the given date/time cannot be represented".to_string());
  }
  Ok(Value::number(t as f64))
}

#[cfg(not(unix))]
fn mktime_impl(
  year: i64,
  month: i64,
  day: i64,
  hour: i64,
  minute: i64,
  seconds: i64,
  _is_dst: Option<bool>,
) -> Result<Value, String> {
  // No local timezone database available portably -- treat the given
  // fields as UTC, same "degrade to UTC" fallback `localtime()` uses on
  // non-Unix, computed via the same civil-calendar math `gmtime()` falls
  // back to there.
  let days = days_from_civil(year, month as u32, day as u32);
  let secs = days * 86400 + hour * 3600 + minute * 60 + seconds;
  Ok(Value::number(secs as f64))
}
