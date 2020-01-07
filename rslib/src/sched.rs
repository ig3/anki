use chrono::{DateTime, Datelike, FixedOffset, Local, TimeZone};

pub struct SchedTimingToday {
    /// The number of days that have passed since the collection was created.
    pub days_elapsed: u32,
    /// Timestamp of the next day rollover.
    pub next_day_at: i64,
}

/// Timing information for the current day.
/// - created is the collection creation time
/// - now is the current UTC timestamp
/// - minutes_west is relative to the local timezone (eg UTC+10 hours is -600)
/// - rollover_hour is the hour of the day the rollover happens (eg 4 for 4am)
pub fn sched_timing_today(
    created: i64,
    now: i64,
    minutes_west: i32,
    rollover_hour: i8,
) -> SchedTimingToday {
    let rollover_today = rollover_for_today(now, minutes_west, rollover_hour).timestamp();

    SchedTimingToday {
        days_elapsed: days_elapsed(created, now, rollover_today),
        next_day_at: rollover_today + 86_400,
    }
}

/// Convert timestamp to the local timezone, with the provided rollover hour.
fn rollover_for_today(
    timestamp: i64,
    minutes_west: i32,
    rollover_hour: i8,
) -> DateTime<FixedOffset> {
    let local_offset = fixed_offset_from_minutes(minutes_west);
    let rollover_hour = normalized_rollover_hour(rollover_hour);
    let dt = local_offset.timestamp(timestamp, 0);
    local_offset
        .ymd(dt.year(), dt.month(), dt.day())
        .and_hms(rollover_hour as u32, 0, 0)
}

/// The number of times the day rolled over between two timestamps.
fn days_elapsed(start: i64, end: i64, rollover_today: i64) -> u32 {
    println!();
    println!("start: {}", start);
    let start_dt = Local.timestamp(start, 0);
    println!("start_dt: {}", start_dt);
    println!("end: {}", end);
    let end_dt = Local.timestamp(end, 0);
    println!("end_dt: {}", end_dt);
    let rollover_dt = Local.timestamp(rollover_today, 0);
    println!("rollover: {}", rollover_today);
    println!("rollover_dt: {}", rollover_dt);

    // get the number of full days that have elapsed
    let secs = (rollover_today - start).max(0);
    let days = (secs / 86_400) as u32;
    println!("days: {}", days);

    // minus one if today's cutoff hasn't passed
    if days > 0 && end < rollover_today {
        days - 1
    } else {
        days
    }
}

/// Negative rollover hours are relative to the next day, eg -1 = 23.
/// Cap hour to 23.
fn normalized_rollover_hour(hour: i8) -> u8 {
    let capped_hour = hour.max(-23).min(23);
    if capped_hour < 0 {
        (24 + capped_hour) as u8
    } else {
        capped_hour as u8
    }
}

/// Build a FixedOffset struct, capping minutes_west if out of bounds.
fn fixed_offset_from_minutes(minutes_west: i32) -> FixedOffset {
    let bounded_minutes = minutes_west.max(-23 * 60).min(23 * 60);
    FixedOffset::west(bounded_minutes * 60)
}

/// Relative to the local timezone, the number of minutes UTC differs by.
/// eg, Australia at +10 hours is -600.
/// Includes the daylight savings offset if applicable.
#[allow(dead_code)]
fn utc_minus_local_mins() -> i32 {
    Local::now().offset().utc_minus_local() / 60
}

#[cfg(test)]
mod test {
    use crate::sched::{
        fixed_offset_from_minutes, normalized_rollover_hour, rollover_for_today,
        sched_timing_today, utc_minus_local_mins,
    };
    use chrono::{Datelike, FixedOffset, TimeZone, Timelike};

    #[test]
    fn test_rollover() {
        assert_eq!(normalized_rollover_hour(4), 4);
        assert_eq!(normalized_rollover_hour(23), 23);
        assert_eq!(normalized_rollover_hour(24), 23);
        assert_eq!(normalized_rollover_hour(-1), 23);
        assert_eq!(normalized_rollover_hour(-2), 22);
        assert_eq!(normalized_rollover_hour(-23), 1);
        assert_eq!(normalized_rollover_hour(-24), 1);

        let now_dt = FixedOffset::west(-600).ymd(2019, 12, 1).and_hms(2, 3, 4);
        let roll_dt = rollover_for_today(now_dt.timestamp(), -600, 4);
        assert_eq!(roll_dt.year(), 2019);
        assert_eq!(roll_dt.month(), 12);
        assert_eq!(roll_dt.day(), 1);
        assert_eq!(roll_dt.hour(), 4);
        assert_eq!(roll_dt.minute(), 0);
        assert_eq!(roll_dt.second(), 0);
    }

    #[test]
    fn test_fixed_offset() {
        let offset = fixed_offset_from_minutes(-600);
        assert_eq!(offset.utc_minus_local(), -600 * 60);
    }

    // helper
    fn elap(start: i64, end: i64, west: i32, rollhour: i8) -> u32 {
        let today = sched_timing_today(start, end, west, rollhour);
        println!("days elapsed: {}", today.days_elapsed);
        today.days_elapsed
    }

    #[test]
    fn test_days_elapsed() {
        std::env::set_var("TZ", "America/Denver");

        let offset = utc_minus_local_mins();

        let created_dt = FixedOffset::west(offset * 60)
            .ymd(2019, 12, 1)
            .and_hms(2, 0, 0);
        let crt = created_dt.timestamp();

        // days can't be negative
        assert_eq!(elap(crt, crt, offset, 4), 0);
        assert_eq!(elap(crt, crt - 86_400, offset, 4), 0);

        // 2am the next day is still the same day
        assert_eq!(elap(crt, crt + 24 * 3600, offset, 4), 0);

        // day rolls over at 4am
        assert_eq!(elap(crt, crt + 26 * 3600, offset, 4), 1);

        // the longest extra delay is +23, or 19 hours past the 4 hour default
        assert_eq!(elap(crt, crt + (26 + 18) * 3600, offset, 23), 0);
        assert_eq!(elap(crt, crt + (26 + 19) * 3600, offset, 23), 1);

        // a collection created @ midnight in MDT in the past
        let mdt = FixedOffset::west(6 * 60 * 60);
        let mst = FixedOffset::west(7 * 60 * 60);
        let crt = mdt.ymd(2018, 8, 6).and_hms(0, 0, 0).timestamp();
        // with the current time being MST
        let now = mst.ymd(2019, 12, 26).and_hms(20, 0, 0).timestamp();
        let offset = mst.utc_minus_local() / 60;
        assert_eq!(elap(crt, now, offset, 4), 507);
        // the previous implementation generated a diferent elapsed number of days with a change
        // to DST, but the number shouldn't change
        let offset = mdt.utc_minus_local() / 60;
        assert_eq!(elap(crt, now, offset, 4), 507);

        // collection created at 3am on the 6th, so day 1 starts at 4am on the 7th, and day 3 on the 9th.
        let crt = mdt.ymd(2018, 8, 6).and_hms(3, 0, 0).timestamp();
        let mst_offset = mst.utc_minus_local() / 60;
        let now = mst.ymd(2018, 8, 9).and_hms(1, 59, 59).timestamp();
        assert_eq!(elap(crt, now, mst_offset, 4), 2);
        let now = mst.ymd(2018, 8, 9).and_hms(3, 59, 59).timestamp();
        assert_eq!(elap(crt, now, mst_offset, 4), 2);
        let now = mst.ymd(2018, 8, 9).and_hms(4, 0, 0).timestamp();
        assert_eq!(elap(crt, now, mst_offset, 4), 3);


        // Test daylight saving time shift handling
        // For TZ America/Denver
        // To MDT 11 Mar 2018 at 2am
        // To MST 4 Nov 2018 at 2am
        println!();
        let crt = mdt.ymd(2018, 10, 29).and_hms(3, 0, 0).timestamp();
        let offset = mdt.utc_minus_local() / 60;
        // Test times up to rollover time on creation date
        let now = mdt.ymd(2018, 10, 29).and_hms(3,0,0).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 0);
        let now = mdt.ymd(2018, 10, 29).and_hms(3,59,59).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 0);
        // For historical reasons, when crt is before rollover time
        // and now is after rollover time on creation date, and up
        // to rollover time of the next day, days_elapsed is 0
        // instead of 1
        let now = mdt.ymd(2018, 10, 29).and_hms(4,0,0).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 0);
        let now = mdt.ymd(2018, 10, 29).and_hms(10,0,0).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 0);
        let now = mdt.ymd(2018, 10, 29).and_hms(23,59,59).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 0);
        let now = mdt.ymd(2018, 10, 30).and_hms(0,0,0).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 0);
        let now = mdt.ymd(2018, 10, 30).and_hms(1,0,0).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 0);
        let now = mdt.ymd(2018, 10, 30).and_hms(2,0,0).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 0);
        let now = mdt.ymd(2018, 10, 30).and_hms(3,0,0).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 0);
        // days elapsed becomes 1 at the rollover time on the day
        // following the creation date and not a second sooner
        let now = mdt.ymd(2018, 10, 30).and_hms(3,59,59).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 0);
        let now = mdt.ymd(2018, 10, 30).and_hms(4,0,0).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 1);
        // days elapsed should increment by 1 each day, from 
        // the rollover time and remain constant until then
        let now = mdt.ymd(2018, 10, 31).and_hms(0,0,0).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 1);
        let now = mdt.ymd(2018, 10, 31).and_hms(1,0,0).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 1);
        let now = mdt.ymd(2018, 10, 31).and_hms(2,0,0).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 1);
        let now = mdt.ymd(2018, 10, 31).and_hms(3,0,0).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 1);
        let now = mdt.ymd(2018, 10, 31).and_hms(3,59,59).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 1);
        let now = mdt.ymd(2018, 10, 31).and_hms(4,0,0).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 2);
        let now = mdt.ymd(2018, 11, 1).and_hms(0,0,0).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 2);
        let now = mdt.ymd(2018, 11, 1).and_hms(3,59,59).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 2);
        let now = mdt.ymd(2018, 11, 1).and_hms(4,0,0).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 3);
        let now = mdt.ymd(2018, 11, 2).and_hms(0,0,0).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 3);
        let now = mdt.ymd(2018, 11, 2).and_hms(3,59,59).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 3);
        let now = mdt.ymd(2018, 11, 2).and_hms(4,0,0).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 4);
        let now = mdt.ymd(2018, 11, 3).and_hms(0,0,0).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 4);
        let now = mdt.ymd(2018, 11, 3).and_hms(3,59,59).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 4);
        let now = mdt.ymd(2018, 11, 3).and_hms(4,0,0).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 5);
        let now = mdt.ymd(2018, 11, 3).and_hms(23,0,0).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 5);
        let now = mdt.ymd(2018, 11, 3).and_hms(23,59,59).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 5);
        // On 4 Nov, switch to MST at 2am
        // Until the switch, days elapsed should be 5
        let now = mdt.ymd(2018, 11, 4).and_hms(0,0,0).timestamp();
        // This test sets new only one second after the previous
        // successful test. The problem is that it is the next day
        // before rollover time but the calculation of days elapsed
        // doesn't recognize the start of the next day until a whole
        // multiple of 86400 seconds after the start time.
        assert_eq!(elap(crt, now, offset, 4), 5);
        let now = mdt.ymd(2018, 11, 4).and_hms(1,0,0).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 5);
        let now = mdt.ymd(2018, 11, 4).and_hms(1,59,59).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 5);
        // Make sure both ends of the fold are correct
        // Test the fold - 2am MDT is 1am MST
        println!("the fold");
        println!("{}", mdt.ymd(2018, 11, 4).and_hms(2,0,0).timestamp());
        println!("{}", mst.ymd(2018, 11, 4).and_hms(1,0,0).timestamp());
        println!("{}", mst.ymd(2018, 11, 4).and_hms(2,0,0).timestamp());
        let now = mdt.ymd(2018, 11, 4).and_hms(2,0,0).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 5);
        let offset = mst.utc_minus_local() / 60;
        let now = mst.ymd(2018, 11, 4).and_hms(1,0,0).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 5);
        // 2am MST is one hour after 2am MDT
        let now = mst.ymd(2018, 11, 4).and_hms(2,0,0).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 5);
        // After the fold, until rollover time
        // days elapsed should remain 5
        let now = mst.ymd(2018, 11, 4).and_hms(3,0,0).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 5);
        // days elapsed should increment to 6 at the rollover time
        // and not a second sooner
        let now = mst.ymd(2018, 11, 4).and_hms(3,59,59).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 5);
        let now = mst.ymd(2018, 11, 4).and_hms(4,0,0).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 6);
        // days elapsed should remain 6 untl rollover on 5 Nov
        let now = mst.ymd(2018, 11, 4).and_hms(23,59,59).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 6);
        let now = mst.ymd(2018, 11, 5).and_hms(0,0,0).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 6);
        let now = mst.ymd(2018, 11, 5).and_hms(1,0,0).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 6);
        let now = mst.ymd(2018, 11, 5).and_hms(2,0,0).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 6);
        let now = mst.ymd(2018, 11, 5).and_hms(3,0,0).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 6);
        let now = mst.ymd(2018, 11, 5).and_hms(3,59,59).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 6);
        let now = mst.ymd(2018, 11, 5).and_hms(4,0,0).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 7);
        // Similarly on subsequent days
        let now = mst.ymd(2018, 11, 5).and_hms(23,59,59).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 7);
        let now = mst.ymd(2018, 11, 6).and_hms(0,0,0).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 7);
        let now = mst.ymd(2018, 11, 6).and_hms(1,0,0).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 7);
        let now = mst.ymd(2018, 11, 6).and_hms(2,0,0).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 7);
        let now = mst.ymd(2018, 11, 6).and_hms(3,0,0).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 7);
        let now = mst.ymd(2018, 11, 6).and_hms(3,59,59).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 7);
        let now = mst.ymd(2018, 11, 6).and_hms(4,0,0).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 8);
        let now = mst.ymd(2018, 11, 6).and_hms(23,59,59).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 8);
        let now = mst.ymd(2018, 11, 7).and_hms(0,0,0).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 8);
        let now = mst.ymd(2018, 11, 7).and_hms(1,0,0).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 8);
        let now = mst.ymd(2018, 11, 7).and_hms(2,0,0).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 8);

        //
        //
        // Now test a few points with crt at rollover time
        println!();
        let crt = mdt.ymd(2018, 10, 29).and_hms(4, 0, 0).timestamp();
        let offset = mdt.utc_minus_local() / 60;
        // Test times up to rollover time on creation date
        let now = mdt.ymd(2018, 10, 29).and_hms(4,0,0).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 0);
        let now = mdt.ymd(2018, 10, 30).and_hms(0,0,0).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 0);
        // It seems that days elapsed doesn't get to 1 until 
        // a full day has elapsed (i.e. the second rollover time
        // at or after creation time). If creation time is just after
        // rollover time, day 0 could be 47 hours long.
        let now = mdt.ymd(2018, 10, 30).and_hms(4,0,0).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 1);
        let now = mdt.ymd(2018, 10, 31).and_hms(0,0,0).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 1);
        let now = mdt.ymd(2018, 10, 31).and_hms(4,0,0).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 2);

        //
        //
        // Now test a few points with crt just after rollover time
        println!();
        let crt = mdt.ymd(2018, 10, 29).and_hms(5, 0, 0).timestamp();
        let offset = mdt.utc_minus_local() / 60;
        // Test times up to rollover time on creation date
        let now = mdt.ymd(2018, 10, 29).and_hms(4,0,0).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 0);
        let now = mdt.ymd(2018, 10, 30).and_hms(0,0,0).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 0);
        let now = mdt.ymd(2018, 10, 30).and_hms(4,0,0).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 0);
        let now = mdt.ymd(2018, 10, 31).and_hms(0,0,0).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 0);
        // With crt an hour after rollover time
        // day 1 doesn't start until almost two days later
        let now = mdt.ymd(2018, 10, 31).and_hms(4,0,0).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 1);
        let now = mdt.ymd(2018, 11, 1).and_hms(0,0,0).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 1);
        let now = mdt.ymd(2018, 11, 1).and_hms(3,59,59).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 1);
        let now = mdt.ymd(2018, 11, 1).and_hms(4,0,0).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 2);
        let now = mdt.ymd(2018, 11, 1).and_hms(4,59,59).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 2);
        let now = mdt.ymd(2018, 11, 1).and_hms(5,0,0).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 2);
        let now = mdt.ymd(2018, 11, 1).and_hms(6,0,0).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 2);
        let now = mdt.ymd(2018, 11, 1).and_hms(7,0,0).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 2);
        let now = mdt.ymd(2018, 11, 1).and_hms(23,59,59).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 2);
        let now = mdt.ymd(2018, 11, 2).and_hms(0,0,0).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 2);

        //
        //
        // Now test a few points with crt at the end of the day
        // Day 1 starts on 31 Oct again. If crt is after rollover
        // time, it makes no difference how much after - day 1 is
        // rollover time two days after creation day.
        //
        println!();
        let crt = mdt.ymd(2018, 10, 29).and_hms(23, 59, 59).timestamp();
        let offset = mdt.utc_minus_local() / 60;
        let now = mdt.ymd(2018, 10, 31).and_hms(3,59,59).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 0);
        let now = mdt.ymd(2018, 10, 31).and_hms(4,0,0).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 1);


        //
        //
        // Test possibly the extreme case of duration of day 0
        //
        println!();
        let crt = mdt.ymd(2018, 10, 29).and_hms(1, 0, 0).timestamp();
        let offset = mdt.utc_minus_local() / 60;
        let now = mdt.ymd(2018, 10, 29).and_hms(0,0,0).timestamp();
        assert_eq!(elap(crt, now, offset, 0), 0);
        let now = mdt.ymd(2018, 10, 30).and_hms(0,0,0).timestamp();
        assert_eq!(elap(crt, now, offset, 0), 0);
        let now = mdt.ymd(2018, 10, 31).and_hms(0,0,0).timestamp();
        assert_eq!(elap(crt, now, offset, 0), 1);
        let now = mdt.ymd(2018, 10, 31).and_hms(1,0,0).timestamp();
        assert_eq!(elap(crt, now, offset, 0), 1);
        let now = mdt.ymd(2018, 10, 31).and_hms(4,0,0).timestamp();
        assert_eq!(elap(crt, now, offset, 0), 1);
        let now = mdt.ymd(2018, 10, 31).and_hms(23,0,0).timestamp();
        assert_eq!(elap(crt, now, offset, 0), 1);
        let now = mdt.ymd(2018, 11, 1).and_hms(0,0,0).timestamp();
        assert_eq!(elap(crt, now, offset, 0), 2);
        let now = mdt.ymd(2018, 11, 1).and_hms(0,59,59).timestamp();
        assert_eq!(elap(crt, now, offset, 0), 2);
        let now = mdt.ymd(2018, 11, 1).and_hms(1,0,0).timestamp();
        assert_eq!(elap(crt, now, offset, 0), 2);


        //
        //
        // Now test a few points with crt after rollover time
        println!();
        let crt = mdt.ymd(2018, 10, 29).and_hms(6, 0, 0).timestamp();
        let offset = mdt.utc_minus_local() / 60;
        // days elapsed should be 0 for any time before creation time
        // But at what point does it become 1? Previous tests suggest
        // after the second rollover time after creation date. The
        // first will be the day after creation day and the second will
        // be the second day after creation day: 31 Oct at 4am.
        let now = mdt.ymd(2018, 10, 29).and_hms(3,0,0).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 0);
        let now = mdt.ymd(2018, 10, 29).and_hms(3,59,59).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 0);
        let now = mdt.ymd(2018, 10, 29).and_hms(4,0,0).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 0);
        let now = mdt.ymd(2018, 10, 29).and_hms(10,0,0).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 0);
        let now = mdt.ymd(2018, 10, 29).and_hms(23,59,59).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 0);
        let now = mdt.ymd(2018, 10, 30).and_hms(0,0,0).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 0);
        let now = mdt.ymd(2018, 10, 30).and_hms(1,0,0).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 0);
        let now = mdt.ymd(2018, 10, 30).and_hms(2,0,0).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 0);
        let now = mdt.ymd(2018, 10, 30).and_hms(3,0,0).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 0);
        let now = mdt.ymd(2018, 10, 30).and_hms(3,59,59).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 0);
        let now = mdt.ymd(2018, 10, 30).and_hms(4,0,0).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 0);
        let now = mdt.ymd(2018, 10, 31).and_hms(0,0,0).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 0);
        let now = mdt.ymd(2018, 10, 31).and_hms(1,0,0).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 0);
        let now = mdt.ymd(2018, 10, 31).and_hms(2,0,0).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 0);
        let now = mdt.ymd(2018, 10, 31).and_hms(3,0,0).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 0);
        let now = mdt.ymd(2018, 10, 31).and_hms(3,59,59).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 0);
        let now = mdt.ymd(2018, 10, 31).and_hms(4,0,0).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 1);
        let now = mdt.ymd(2018, 11, 1).and_hms(0,0,0).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 1);
        let now = mdt.ymd(2018, 11, 1).and_hms(3,59,59).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 1);
        let now = mdt.ymd(2018, 11, 1).and_hms(4,0,0).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 2);
        let now = mdt.ymd(2018, 11, 2).and_hms(0,0,0).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 2);
        let now = mdt.ymd(2018, 11, 2).and_hms(3,59,59).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 2);
        let now = mdt.ymd(2018, 11, 2).and_hms(4,0,0).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 3);
        let now = mdt.ymd(2018, 11, 2).and_hms(6,0,0).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 3);
        let now = mdt.ymd(2018, 11, 2).and_hms(23,59,59).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 3);
        let now = mdt.ymd(2018, 11, 3).and_hms(0,0,0).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 3);
        let now = mdt.ymd(2018, 11, 3).and_hms(3,59,59).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 3);
        let now = mdt.ymd(2018, 11, 3).and_hms(4,0,0).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 4);
        let now = mdt.ymd(2018, 11, 3).and_hms(23,59,59).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 4);
        // On 4 Nov, switch to MST at 2am
        // Until the switch, days elapsed should be 5
        let now = mdt.ymd(2018, 11, 4).and_hms(0,0,0).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 4);
        let now = mdt.ymd(2018, 11, 4).and_hms(1,0,0).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 4);
        let now = mdt.ymd(2018, 11, 4).and_hms(1,59,59).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 4);
        // Make sure both ends of the fold are correct
        // Test the fold - 2am MDT is 1am MST
        let now = mdt.ymd(2018, 11, 4).and_hms(2,0,0).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 4);
        let offset = mst.utc_minus_local() / 60;
        let now = mst.ymd(2018, 11, 4).and_hms(1,0,0).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 4);
        // 2am MST is one hour after 2am MDT
        let now = mst.ymd(2018, 11, 4).and_hms(2,0,0).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 4);
        // After the fold, until rollover time
        // days elapsed should remain 4
        let now = mst.ymd(2018, 11, 4).and_hms(3,0,0).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 4);
        // days elapsed should increment to 6 at the rollover time
        // and not a second sooner
        let now = mst.ymd(2018, 11, 4).and_hms(3,59,59).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 4);
        let now = mst.ymd(2018, 11, 4).and_hms(4,0,0).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 5);
        let now = mst.ymd(2018, 11, 4).and_hms(4,59,59).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 5);
        let now = mst.ymd(2018, 11, 4).and_hms(5,0,0).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 5);
        let now = mst.ymd(2018, 11, 4).and_hms(6,0,0).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 5);
        // days elapsed should remain 6 untl rollover on 5 Nov
        let now = mst.ymd(2018, 11, 4).and_hms(23,59,59).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 5);
        let now = mst.ymd(2018, 11, 5).and_hms(0,0,0).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 5);
        let now = mst.ymd(2018, 11, 5).and_hms(1,0,0).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 5);
        let now = mst.ymd(2018, 11, 5).and_hms(2,0,0).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 5);
        let now = mst.ymd(2018, 11, 5).and_hms(3,0,0).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 5);
        let now = mst.ymd(2018, 11, 5).and_hms(3,59,59).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 5);
        let now = mst.ymd(2018, 11, 5).and_hms(4,0,0).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 6);
        // Similarly on subsequent days
        let now = mst.ymd(2018, 11, 5).and_hms(23,59,59).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 6);
        let now = mst.ymd(2018, 11, 6).and_hms(0,0,0).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 6);
        let now = mst.ymd(2018, 11, 6).and_hms(1,0,0).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 6);
        let now = mst.ymd(2018, 11, 6).and_hms(2,0,0).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 6);
        let now = mst.ymd(2018, 11, 6).and_hms(3,0,0).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 6);
        let now = mst.ymd(2018, 11, 6).and_hms(3,59,59).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 6);
        let now = mst.ymd(2018, 11, 6).and_hms(4,0,0).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 7);
        let now = mst.ymd(2018, 11, 6).and_hms(23,59,59).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 7);
        let now = mst.ymd(2018, 11, 7).and_hms(0,0,0).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 7);
        let now = mst.ymd(2018, 11, 7).and_hms(1,0,0).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 7);
        let now = mst.ymd(2018, 11, 7).and_hms(2,0,0).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 7);

        //
        //
        // Test transition from MST to MDT
        //
        println!();
        let crt = mst.ymd(2019, 3, 3).and_hms(4, 0, 0).timestamp();
        let offset = mst.utc_minus_local() / 60;
        let now = mst.ymd(2019, 3, 3).and_hms(4,0,0).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 0);
        let now = mst.ymd(2019, 3, 4).and_hms(4,0,0).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 1);
        let now = mst.ymd(2019, 3, 5).and_hms(4,0,0).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 2);
        let now = mst.ymd(2019, 3, 6).and_hms(4,0,0).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 3);
        let now = mst.ymd(2019, 3, 9).and_hms(4,0,0).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 6);

        // On 10 Mar, switch to MDT at 2am
        // Until the switch, days elapsed should be 5
        let now = mst.ymd(2019, 3, 10).and_hms(0,0,0).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 6);
        let now = mst.ymd(2019, 3, 10).and_hms(1,0,0).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 6);
        let now = mst.ymd(2019, 3, 10).and_hms(2,0,0).timestamp();
        assert_eq!(elap(crt, now, offset, 4), 6);
        let offset = mdt.utc_minus_local() / 60;
        let now = mdt.ymd(2019, 3, 10).and_hms(3,0,0).timestamp();
        // This fails - returning 5 instead of 6
        // assert_eq!(elap(crt, now, offset, 4), 6);
        let now = mdt.ymd(2019, 3, 10).and_hms(3,59,59).timestamp();
        // This fails - returning 5 instead of 6
        // assert_eq!(elap(crt, now, offset, 4), 6);
        let now = mdt.ymd(2019, 3, 10).and_hms(4,0,0).timestamp();
        // This fails - returning 6 instead of 7
        // assert_eq!(elap(crt, now, offset, 4), 7);
        let now = mdt.ymd(2019, 3, 10).and_hms(5,0,0).timestamp();
        // This fails - returning 6 instead of 7
        // assert_eq!(elap(crt, now, offset, 4), 7);
        let now = mdt.ymd(2019, 3, 11).and_hms(4,0,0).timestamp();
        // This fails - returning 7 instead of 8
        // assert_eq!(elap(crt, now, offset, 4), 8);
        let now = mdt.ymd(2019, 3, 12).and_hms(4,0,0).timestamp();
        // This fails - returning 8 instead of 9
        // assert_eq!(elap(crt, now, offset, 4), 9);



        // sure to fail
        assert_eq!(111, 222);

    }
}
