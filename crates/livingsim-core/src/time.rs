//! Simulated time. 1 tick = 1 simulated minute.
//!
//! Tick 0 is **Year 1, Monday January 1st, 06:00**. Years have 365 days, weeks 7 days, and
//! there are no leap years. The real-time mapping (1 tick = 2.5 s) lives outside the core.

pub type Tick = u32;

pub const TICKS_PER_HOUR: Tick = 60;
pub const TICKS_PER_DAY: Tick = 1440;
pub const DAYS_PER_YEAR: u32 = 365;
pub const TICKS_PER_YEAR: u64 = TICKS_PER_DAY as u64 * DAYS_PER_YEAR as u64;
/// Minute of the day at tick 0.
pub const START_MINUTE: u32 = 6 * 60;
/// Real milliseconds per tick (used by the runner and the browser, never by the core).
pub const REAL_MS_PER_TICK: u64 = 2500;

/// Minutes since Year 1, Jan 1st 00:00.
#[inline]
pub fn absolute_minute(t: Tick) -> u64 {
    t as u64 + START_MINUTE as u64
}

/// Minute of the day, 0..1440.
#[inline]
pub fn minute_of_day(t: Tick) -> u32 {
    (absolute_minute(t) % TICKS_PER_DAY as u64) as u32
}

/// Days since Year 1, Jan 1st.
#[inline]
pub fn day_number(t: Tick) -> u32 {
    (absolute_minute(t) / TICKS_PER_DAY as u64) as u32
}

/// 0 = Monday … 6 = Sunday.
#[inline]
pub fn weekday(t: Tick) -> u8 {
    (day_number(t) % 7) as u8
}

/// The first tick `>= t` whose minute of the day is `m`.
#[inline]
pub fn next_minute_of_day(t: Tick, m: u32) -> Tick {
    let now = minute_of_day(t);
    let delta = if m >= now {
        m - now
    } else {
        TICKS_PER_DAY - now + m
    };
    t + delta
}

/// The tick at which the calendar day containing `t` started (00:00), saturating at 0.
#[inline]
pub fn start_of_day(t: Tick) -> i64 {
    t as i64 - minute_of_day(t) as i64
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Date {
    pub year: u32,
    /// 1-based.
    pub day_of_year: u32,
    pub month: u8,
    pub day: u8,
    pub weekday: u8,
    pub hour: u8,
    pub minute: u8,
}

const MONTH_DAYS: [u32; 12] = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
pub const MONTH_NAMES: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];
pub const WEEKDAY_NAMES: [&str; 7] = [
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
    "Sunday",
];

pub fn date(t: Tick) -> Date {
    let day = day_number(t);
    let year = day / DAYS_PER_YEAR + 1;
    let mut doy = day % DAYS_PER_YEAR;
    let mut month = 0;
    while doy >= MONTH_DAYS[month] {
        doy -= MONTH_DAYS[month];
        month += 1;
    }
    let m = minute_of_day(t);
    Date {
        year,
        day_of_year: day % DAYS_PER_YEAR + 1,
        month: month as u8 + 1,
        day: doy as u8 + 1,
        weekday: weekday(t),
        hour: (m / 60) as u8,
        minute: (m % 60) as u8,
    }
}

/// Age in whole years of somebody born at `birth` (may be negative: before tick 0).
#[inline]
pub fn age_years(birth: i64, t: Tick) -> u32 {
    let d = t as i64 - birth;
    if d <= 0 {
        0
    } else {
        (d as u64 / TICKS_PER_YEAR) as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calendar() {
        let d = date(0);
        assert_eq!(
            (d.year, d.month, d.day, d.weekday, d.hour, d.minute),
            (1, 1, 1, 0, 6, 0)
        );
        let d = date(18 * 60); // midnight
        assert_eq!((d.day, d.weekday, d.hour), (2, 1, 0));
        let d = date(TICKS_PER_DAY * 365);
        assert_eq!((d.year, d.month, d.day), (2, 1, 1));
        assert_eq!(next_minute_of_day(0, 6 * 60), 0);
        assert_eq!(next_minute_of_day(0, 5 * 60), 23 * 60);
        assert_eq!(age_years(-(TICKS_PER_YEAR as i64) * 30, 0), 30);
    }
}
