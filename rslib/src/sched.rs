use chrono::{Date, Duration, FixedOffset, Local, TimeZone};

pub struct SchedTimingToday {
    /// The number of days that have passed since the collection was created.
    pub days_elapsed: u32,
    /// Timestamp of the next day rollover.
    pub next_day_at: i64,
}

/// Timing information for the current day.
/// - created_secs is a UNIX timestamp of the collection creation time
/// - created_mins_west is the offset west of UTC at the time of creation
///   (eg UTC+10 hours is -600)
/// - now_secs is a timestamp of the current time
/// - now_mins_west is the current offset west of UTC
/// - rollover_hour is the hour of the day the rollover happens (eg 4 for 4am)
pub fn sched_timing_today(
    created_secs: i64,
    created_mins_west: i32,
    now_secs: i64,
    now_mins_west: i32,
    rollover_hour: i8,
) -> SchedTimingToday {
    // get date(times) based on timezone offsets
    let created_date = fixed_offset_from_minutes(created_mins_west)
        .timestamp(created_secs, 0)
        .date();
    let now_datetime = fixed_offset_from_minutes(now_mins_west).timestamp(now_secs, 0);
    let today = now_datetime.date();

    // rollover
    let rollover_hour = normalized_rollover_hour(rollover_hour);
    let rollover_today_datetime = today.and_hms(rollover_hour as u32, 0, 0);
    let rollover_passed = rollover_today_datetime <= now_datetime;
    let next_day_at = (rollover_today_datetime + Duration::days(1)).timestamp();

    // day count
    let days_elapsed = days_elapsed(created_date, today, rollover_passed);

    SchedTimingToday {
        days_elapsed,
        next_day_at,
    }
}

/// The number of times the day rolled over between two timestamps.
///  fn days_elapsed_test(start: i64, end: i64, rollover_today: i64) -> u32 {
///      println!();
///      println!("start: {}", start);
///      let start_dt = Local.timestamp(start, 0);
///      println!("start_dt: {}", start_dt);
///      println!("end: {}", end);
///      let end_dt = Local.timestamp(end, 0);
///      println!("end_dt: {}", end_dt);
///      let rollover_dt = Local.timestamp(rollover_today, 0);
///      println!("rollover: {}", rollover_today);
///      println!("rollover_dt: {}", rollover_dt);
///  
///      let rollover_dt = Local.timestamp(rollover_today, 0);
///      println!("rollover hour: {}", rollover_dt.hour());
///  
///      let start_dt = Local.timestamp(start, 0);
///  
///      let reference_dt = Local
///          .ymd(start_dt.year(), start_dt.month(), start_dt.day())
///          .and_hms(rollover_dt.hour(), 0, 0);
///  
///      println!("reference time: {}", reference_dt);
///  
///      let reference = reference_dt.timestamp() - 3601;
///  
///  
///      println!("reference: {}", reference);
///      let x = Local.timestamp(reference, 0);
///      println!("actual reference time: {}", x);
///  
///  
///      // get the number of full days that have elapsed
///      let secs = (rollover_today - reference).max(0);
///      let days = (secs / 86_400) as u32;
///      println!("days: {} leaving {}", days, (secs % 86_400));
///  
///      // minus one if today's cutoff hasn't passed
///      if days > 0 && end < rollover_today {
///          days - 1
///      } else {
///          days
///      }
///  }

/// The number of times the day rolled over between two dates.
fn days_elapsed(
    start_date: Date<FixedOffset>,
    end_date: Date<FixedOffset>,
    rollover_passed: bool,
) -> u32 {
    let days = (end_date - start_date).num_days();

    // current day doesn't count before rollover time
    let days = if rollover_passed { days } else { days - 1 };

    // minimum of 0
    days.max(0) as u32
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
        fixed_offset_from_minutes, normalized_rollover_hour, sched_timing_today,
        utc_minus_local_mins,
    };
    use chrono::{Duration, FixedOffset, Local, Timelike, TimeZone};

    #[test]
    fn test_rollover() {
        assert_eq!(normalized_rollover_hour(4), 4);
        assert_eq!(normalized_rollover_hour(23), 23);
        assert_eq!(normalized_rollover_hour(24), 23);
        assert_eq!(normalized_rollover_hour(-1), 23);
        assert_eq!(normalized_rollover_hour(-2), 22);
        assert_eq!(normalized_rollover_hour(-23), 1);
        assert_eq!(normalized_rollover_hour(-24), 1);
    }

    #[test]
    fn test_fixed_offset() {
        let offset = fixed_offset_from_minutes(-600);
        assert_eq!(offset.utc_minus_local(), -600 * 60);
    }

    // helper
    fn elap(start: i64, end: i64, start_west: i32, end_west: i32, rollhour: i8) -> u32 {
        let today = sched_timing_today(start, start_west, end, end_west, rollhour);
        today.days_elapsed
    }

    #[test]
    fn test_days_elapsed() {
        std::env::set_var("TZ", "America/Denver");
        let local_offset = utc_minus_local_mins();

        let created_dt = FixedOffset::west(local_offset * 60)
            .ymd(2019, 12, 1)
            .and_hms(2, 0, 0);
        let crt = created_dt.timestamp();

        // days can't be negative
        assert_eq!(elap(crt, crt, local_offset, local_offset, 4), 0);
        assert_eq!(elap(crt, crt - 86_400, local_offset, local_offset, 4), 0);

        // 2am the next day is still the same day
        assert_eq!(elap(crt, crt + 24 * 3600, local_offset, local_offset, 4), 0);

        // day rolls over at 4am
        assert_eq!(elap(crt, crt + 26 * 3600, local_offset, local_offset, 4), 1);

        // the longest extra delay is +23, or 19 hours past the 4 hour default
        assert_eq!(
            elap(crt, crt + (26 + 18) * 3600, local_offset, local_offset, 23),
            0
        );
        assert_eq!(
            elap(crt, crt + (26 + 19) * 3600, local_offset, local_offset, 23),
            1
        );

        let mdt = FixedOffset::west(6 * 60 * 60);
        let mdt_offset = mdt.utc_minus_local() / 60;
        let mst = FixedOffset::west(7 * 60 * 60);
        let mst_offset = mst.utc_minus_local() / 60;

        // a collection created @ midnight in MDT in the past
        let crt = mdt.ymd(2018, 8, 6).and_hms(0, 0, 0).timestamp();
        // with the current time being MST
        let now = mst.ymd(2019, 12, 26).and_hms(20, 0, 0).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mst_offset, 4), 507);
        // the previous implementation generated a diferent elapsed number of days with a change
        // to DST, but the number shouldn't change
        assert_eq!(elap(crt, now, mdt_offset, mdt_offset, 4), 507);

        // collection created at 3am on the 6th, so day 1 starts at 4am on the 7th, and day 3 on the 9th.
        let crt = mdt.ymd(2018, 8, 6).and_hms(3, 0, 0).timestamp();
        let now = mst.ymd(2018, 8, 9).and_hms(1, 59, 59).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mst_offset, 4), 2);
        let now = mst.ymd(2018, 8, 9).and_hms(3, 59, 59).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mst_offset, 4), 2);
        let now = mst.ymd(2018, 8, 9).and_hms(4, 0, 0).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mst_offset, 4), 3);

        // try a bunch of combinations of creation time, current time, and rollover hour
        let hours_of_interest = &[0, 1, 4, 12, 22, 23];
        for creation_hour in hours_of_interest {
            let crt_dt = mdt.ymd(2018, 8, 6).and_hms(*creation_hour, 0, 0);
            let crt_stamp = crt_dt.timestamp();
            let crt_offset = mdt_offset;

            for current_day in 0..=3 {
                for current_hour in hours_of_interest {
                    for rollover_hour in hours_of_interest {
                        let end_dt = mdt
                            .ymd(2018, 8, 6 + current_day)
                            .and_hms(*current_hour, 0, 0);
                        let end_stamp = end_dt.timestamp();
                        let end_offset = mdt_offset;
                        let elap_day = if *current_hour < *rollover_hour {
                            current_day.max(1) - 1
                        } else {
                            current_day
                        };

                        assert_eq!(
                            elap(
                                crt_stamp,
                                end_stamp,
                                crt_offset,
                                end_offset,
                                *rollover_hour as i8
                            ),
                            elap_day
                        );
                    }
                }
            }
        }

        // Ian Goodacre tests start here
        // Test daylight saving time shift handling
        // For TZ America/Denver
        // To MDT 11 Mar 2018 at 2am
        // To MST 4 Nov 2018 at 2am
        println!();
        let crt = mdt.ymd(2018, 10, 29).and_hms(3, 0, 0).timestamp();
        let offset = mdt.utc_minus_local() / 60;
        // Test times up to rollover time on creation date
        let now = mdt.ymd(2018, 10, 29).and_hms(3,0,0).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mdt_offset, 4), 0);
        let now = mdt.ymd(2018, 10, 29).and_hms(3,59,59).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mdt_offset, 4), 0);
        // For historical reasons, when crt is before rollover time
        // and now is after rollover time on creation date, and up
        // to rollover time of the next day, days_elapsed is 0
        // instead of 1
        let now = mdt.ymd(2018, 10, 29).and_hms(4,0,0).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mdt_offset, 4), 0);
        let now = mdt.ymd(2018, 10, 29).and_hms(10,0,0).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mdt_offset, 4), 0);
        let now = mdt.ymd(2018, 10, 29).and_hms(23,59,59).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mdt_offset, 4), 0);
        let now = mdt.ymd(2018, 10, 30).and_hms(0,0,0).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mdt_offset, 4), 0);
        let now = mdt.ymd(2018, 10, 30).and_hms(1,0,0).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mdt_offset, 4), 0);
        let now = mdt.ymd(2018, 10, 30).and_hms(2,0,0).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mdt_offset, 4), 0);
        let now = mdt.ymd(2018, 10, 30).and_hms(3,0,0).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mdt_offset, 4), 0);
        // days elapsed becomes 1 at the rollover time on the day
        // following the creation date and not a second sooner
        let now = mdt.ymd(2018, 10, 30).and_hms(3,59,59).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mdt_offset, 4), 0);
        let now = mdt.ymd(2018, 10, 30).and_hms(4,0,0).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mdt_offset, 4), 1);
        // days elapsed should increment by 1 each day, from 
        // the rollover time and remain constant until then
        let now = mdt.ymd(2018, 10, 31).and_hms(0,0,0).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mdt_offset, 4), 1);
        let now = mdt.ymd(2018, 10, 31).and_hms(1,0,0).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mdt_offset, 4), 1);
        let now = mdt.ymd(2018, 10, 31).and_hms(2,0,0).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mdt_offset, 4), 1);
        let now = mdt.ymd(2018, 10, 31).and_hms(3,0,0).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mdt_offset, 4), 1);
        let now = mdt.ymd(2018, 10, 31).and_hms(3,59,59).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mdt_offset, 4), 1);
        let now = mdt.ymd(2018, 10, 31).and_hms(4,0,0).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mdt_offset, 4), 2);
        let now = mdt.ymd(2018, 11, 1).and_hms(0,0,0).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mdt_offset, 4), 2);
        let now = mdt.ymd(2018, 11, 1).and_hms(3,59,59).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mdt_offset, 4), 2);
        let now = mdt.ymd(2018, 11, 1).and_hms(4,0,0).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mdt_offset, 4), 3);
        let now = mdt.ymd(2018, 11, 2).and_hms(0,0,0).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mdt_offset, 4), 3);
        let now = mdt.ymd(2018, 11, 2).and_hms(3,59,59).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mdt_offset, 4), 3);
        let now = mdt.ymd(2018, 11, 2).and_hms(4,0,0).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mdt_offset, 4), 4);
        let now = mdt.ymd(2018, 11, 3).and_hms(0,0,0).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mdt_offset, 4), 4);
        let now = mdt.ymd(2018, 11, 3).and_hms(3,59,59).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mdt_offset, 4), 4);
        let now = mdt.ymd(2018, 11, 3).and_hms(4,0,0).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mdt_offset, 4), 5);
        let now = mdt.ymd(2018, 11, 3).and_hms(23,0,0).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mdt_offset, 4), 5);
        let now = mdt.ymd(2018, 11, 3).and_hms(23,59,59).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mdt_offset, 4), 5);
        // On 4 Nov, switch to MST at 2am
        // Until the switch, days elapsed should be 5
        let now = mdt.ymd(2018, 11, 4).and_hms(0,0,0).timestamp();
        // This test sets new only one second after the previous
        // successful test. The problem is that it is the next day
        // before rollover time but the calculation of days elapsed
        // doesn't recognize the start of the next day until a whole
        // multiple of 86400 seconds after the start time.
        assert_eq!(elap(crt, now, mdt_offset, mdt_offset, 4), 5);
        let now = mdt.ymd(2018, 11, 4).and_hms(1,0,0).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mdt_offset, 4), 5);
        let now = mdt.ymd(2018, 11, 4).and_hms(1,59,59).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mdt_offset, 4), 5);
        // Make sure both ends of the fold are correct
        // Test the fold - 2am MDT is 1am MST
        println!("the fold");
        println!("{}", mdt.ymd(2018, 11, 4).and_hms(2,0,0).timestamp());
        println!("{}", mst.ymd(2018, 11, 4).and_hms(1,0,0).timestamp());
        println!("{}", mst.ymd(2018, 11, 4).and_hms(2,0,0).timestamp());
        let now = mdt.ymd(2018, 11, 4).and_hms(2,0,0).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mdt_offset, 4), 5);
        let offset = mst.utc_minus_local() / 60;
        let now = mst.ymd(2018, 11, 4).and_hms(1,0,0).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mst_offset, 4), 5);
        // 2am MST is one hour after 2am MDT
        let now = mst.ymd(2018, 11, 4).and_hms(2,0,0).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mst_offset, 4), 5);
        // After the fold, until rollover time
        // days elapsed should remain 5
        let now = mst.ymd(2018, 11, 4).and_hms(3,0,0).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mst_offset, 4), 5);
        // days elapsed should increment to 6 at the rollover time
        // and not a second sooner
        let now = mst.ymd(2018, 11, 4).and_hms(3,59,59).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mst_offset, 4), 5);
        let now = mst.ymd(2018, 11, 4).and_hms(4,0,0).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mst_offset, 4), 6);
        // days elapsed should remain 6 untl rollover on 5 Nov
        let now = mst.ymd(2018, 11, 4).and_hms(23,59,59).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mst_offset, 4), 6);
        let now = mst.ymd(2018, 11, 5).and_hms(0,0,0).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mst_offset, 4), 6);
        let now = mst.ymd(2018, 11, 5).and_hms(1,0,0).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mst_offset, 4), 6);
        let now = mst.ymd(2018, 11, 5).and_hms(2,0,0).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mst_offset, 4), 6);
        let now = mst.ymd(2018, 11, 5).and_hms(3,0,0).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mst_offset, 4), 6);
        let now = mst.ymd(2018, 11, 5).and_hms(3,59,59).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mst_offset, 4), 6);
        let now = mst.ymd(2018, 11, 5).and_hms(4,0,0).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mst_offset, 4), 7);
        // Similarly on subsequent days
        let now = mst.ymd(2018, 11, 5).and_hms(23,59,59).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mst_offset, 4), 7);
        let now = mst.ymd(2018, 11, 6).and_hms(0,0,0).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mst_offset, 4), 7);
        let now = mst.ymd(2018, 11, 6).and_hms(1,0,0).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mst_offset, 4), 7);
        let now = mst.ymd(2018, 11, 6).and_hms(2,0,0).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mst_offset, 4), 7);
        let now = mst.ymd(2018, 11, 6).and_hms(3,0,0).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mst_offset, 4), 7);
        let now = mst.ymd(2018, 11, 6).and_hms(3,59,59).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mst_offset, 4), 7);
        let now = mst.ymd(2018, 11, 6).and_hms(4,0,0).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mst_offset, 4), 8);
        let now = mst.ymd(2018, 11, 6).and_hms(23,59,59).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mst_offset, 4), 8);
        let now = mst.ymd(2018, 11, 7).and_hms(0,0,0).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mst_offset, 4), 8);
        let now = mst.ymd(2018, 11, 7).and_hms(1,0,0).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mst_offset, 4), 8);
        let now = mst.ymd(2018, 11, 7).and_hms(2,0,0).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mst_offset, 4), 8);

        //
        //
        // Now test a few points with crt at rollover time
        println!();
        let crt = mdt.ymd(2018, 10, 29).and_hms(4, 0, 0).timestamp();
        let offset = mdt.utc_minus_local() / 60;
        // Test times up to rollover time on creation date
        let now = mdt.ymd(2018, 10, 29).and_hms(4,0,0).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mdt_offset, 4), 0);
        let now = mdt.ymd(2018, 10, 30).and_hms(0,0,0).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mdt_offset, 4), 0);
        // It seems that days elapsed doesn't get to 1 until 
        // a full day has elapsed (i.e. the second rollover time
        // at or after creation time). If creation time is just after
        // rollover time, day 0 could be 47 hours long.
        let now = mdt.ymd(2018, 10, 30).and_hms(4,0,0).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mdt_offset, 4), 1);
        let now = mdt.ymd(2018, 10, 31).and_hms(0,0,0).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mdt_offset, 4), 1);
        let now = mdt.ymd(2018, 10, 31).and_hms(4,0,0).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mdt_offset, 4), 2);

        //
        //
        // Now test a few points with crt just after rollover time
        println!();
        let crt = mdt.ymd(2018, 10, 29).and_hms(5, 0, 0).timestamp();
        let offset = mdt.utc_minus_local() / 60;
        // Test times up to rollover time on creation date
        let now = mdt.ymd(2018, 10, 29).and_hms(4,0,0).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mdt_offset, 4), 0);
        let now = mdt.ymd(2018, 10, 30).and_hms(0,0,0).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mdt_offset, 4), 0);
        let now = mdt.ymd(2018, 10, 30).and_hms(4,0,0).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mdt_offset, 4), 1);
        let now = mdt.ymd(2018, 10, 31).and_hms(0,0,0).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mdt_offset, 4), 1);
        // With crt an hour after rollover time
        // day 1 doesn't start until almost two days later
        let now = mdt.ymd(2018, 10, 31).and_hms(4,0,0).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mdt_offset, 4), 2);
        let now = mdt.ymd(2018, 11, 1).and_hms(0,0,0).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mdt_offset, 4), 2);
        let now = mdt.ymd(2018, 11, 1).and_hms(3,59,59).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mdt_offset, 4), 2);
        let now = mdt.ymd(2018, 11, 1).and_hms(4,0,0).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mdt_offset, 4), 3);
        let now = mdt.ymd(2018, 11, 1).and_hms(4,59,59).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mdt_offset, 4), 3);
        let now = mdt.ymd(2018, 11, 1).and_hms(5,0,0).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mdt_offset, 4), 3);
        let now = mdt.ymd(2018, 11, 1).and_hms(6,0,0).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mdt_offset, 4), 3);
        let now = mdt.ymd(2018, 11, 1).and_hms(7,0,0).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mdt_offset, 4), 3);
        let now = mdt.ymd(2018, 11, 1).and_hms(23,59,59).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mdt_offset, 4), 3);
        let now = mdt.ymd(2018, 11, 2).and_hms(0,0,0).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mdt_offset, 4), 3);

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
        let now = mdt.ymd(2018, 10, 30).and_hms(3,59,59).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mdt_offset, 4), 0);
        let now = mdt.ymd(2018, 10, 30).and_hms(4,0,0).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mdt_offset, 4), 1);


        //
        //
        // Test possibly the extreme case of duration of day 0
        //
        println!();
        let crt = mdt.ymd(2018, 10, 29).and_hms(1, 0, 0).timestamp();
        let offset = mdt.utc_minus_local() / 60;
        let now = mdt.ymd(2018, 10, 29).and_hms(0,0,0).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mdt_offset, 0), 0);
        let now = mdt.ymd(2018, 10, 29).and_hms(23,59,59).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mdt_offset, 0), 0);
        let now = mdt.ymd(2018, 10, 30).and_hms(0,0,0).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mdt_offset, 0), 1);
        let now = mdt.ymd(2018, 10, 31).and_hms(0,0,0).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mdt_offset, 0), 2);
        let now = mdt.ymd(2018, 10, 31).and_hms(1,0,0).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mdt_offset, 0), 2);
        let now = mdt.ymd(2018, 10, 31).and_hms(4,0,0).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mdt_offset, 0), 2);
        let now = mdt.ymd(2018, 10, 31).and_hms(23,0,0).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mdt_offset, 0), 2);
        let now = mdt.ymd(2018, 11, 1).and_hms(0,0,0).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mdt_offset, 0), 3);
        let now = mdt.ymd(2018, 11, 1).and_hms(0,59,59).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mdt_offset, 0), 3);
        let now = mdt.ymd(2018, 11, 1).and_hms(1,0,0).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mdt_offset, 0), 3);


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
        assert_eq!(elap(crt, now, mdt_offset, mdt_offset, 4), 0);
        let now = mdt.ymd(2018, 10, 29).and_hms(3,59,59).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mdt_offset, 4), 0);
        let now = mdt.ymd(2018, 10, 29).and_hms(4,0,0).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mdt_offset, 4), 0);
        let now = mdt.ymd(2018, 10, 29).and_hms(10,0,0).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mdt_offset, 4), 0);
        let now = mdt.ymd(2018, 10, 29).and_hms(23,59,59).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mdt_offset, 4), 0);
        let now = mdt.ymd(2018, 10, 30).and_hms(0,0,0).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mdt_offset, 4), 0);
        let now = mdt.ymd(2018, 10, 30).and_hms(1,0,0).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mdt_offset, 4), 0);
        let now = mdt.ymd(2018, 10, 30).and_hms(2,0,0).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mdt_offset, 4), 0);
        let now = mdt.ymd(2018, 10, 30).and_hms(3,0,0).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mdt_offset, 4), 0);
        let now = mdt.ymd(2018, 10, 30).and_hms(3,59,59).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mdt_offset, 4), 0);
        let now = mdt.ymd(2018, 10, 30).and_hms(4,0,0).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mdt_offset, 4), 1);
        let now = mdt.ymd(2018, 10, 31).and_hms(0,0,0).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mdt_offset, 4), 1);
        let now = mdt.ymd(2018, 10, 31).and_hms(1,0,0).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mdt_offset, 4), 1);
        let now = mdt.ymd(2018, 10, 31).and_hms(2,0,0).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mdt_offset, 4), 1);
        let now = mdt.ymd(2018, 10, 31).and_hms(3,0,0).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mdt_offset, 4), 1);
        let now = mdt.ymd(2018, 10, 31).and_hms(3,59,59).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mdt_offset, 4), 1);
        let now = mdt.ymd(2018, 10, 31).and_hms(4,0,0).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mdt_offset, 4), 2);
        let now = mdt.ymd(2018, 11, 1).and_hms(0,0,0).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mdt_offset, 4), 2);
        let now = mdt.ymd(2018, 11, 1).and_hms(3,59,59).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mdt_offset, 4), 2);
        let now = mdt.ymd(2018, 11, 1).and_hms(4,0,0).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mdt_offset, 4), 3);
        let now = mdt.ymd(2018, 11, 2).and_hms(0,0,0).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mdt_offset, 4), 3);
        let now = mdt.ymd(2018, 11, 2).and_hms(3,59,59).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mdt_offset, 4), 3);
        let now = mdt.ymd(2018, 11, 2).and_hms(4,0,0).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mdt_offset, 4), 4);
        let now = mdt.ymd(2018, 11, 2).and_hms(6,0,0).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mdt_offset, 4), 4);
        let now = mdt.ymd(2018, 11, 2).and_hms(23,59,59).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mdt_offset, 4), 4);
        let now = mdt.ymd(2018, 11, 3).and_hms(0,0,0).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mdt_offset, 4), 4);
        let now = mdt.ymd(2018, 11, 3).and_hms(3,59,59).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mdt_offset, 4), 4);
        let now = mdt.ymd(2018, 11, 3).and_hms(4,0,0).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mdt_offset, 4), 5);
        let now = mdt.ymd(2018, 11, 3).and_hms(23,59,59).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mdt_offset, 4), 5);
        // On 4 Nov, switch to MST at 2am
        // Until the switch, days elapsed should be 5
        let now = mdt.ymd(2018, 11, 4).and_hms(0,0,0).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mdt_offset, 4), 5);
        let now = mdt.ymd(2018, 11, 4).and_hms(1,0,0).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mdt_offset, 4), 5);
        let now = mdt.ymd(2018, 11, 4).and_hms(1,59,59).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mdt_offset, 4), 5);
        // Make sure both ends of the fold are correct
        // Test the fold - 2am MDT is 1am MST
        let now = mdt.ymd(2018, 11, 4).and_hms(2,0,0).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mdt_offset, 4), 5);
        let offset = mst.utc_minus_local() / 60;
        let now = mst.ymd(2018, 11, 4).and_hms(1,0,0).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mst_offset, 4), 5);
        // 2am MST is one hour after 2am MDT
        let now = mst.ymd(2018, 11, 4).and_hms(2,0,0).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mst_offset, 4), 5);
        // After the fold, until rollover time
        // days elapsed should remain 4
        let now = mst.ymd(2018, 11, 4).and_hms(3,0,0).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mst_offset, 4), 5);
        // days elapsed should increment to 6 at the rollover time
        // and not a second sooner
        let now = mst.ymd(2018, 11, 4).and_hms(3,59,59).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mst_offset, 4), 5);
        let now = mst.ymd(2018, 11, 4).and_hms(4,0,0).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mst_offset, 4), 6);
        let now = mst.ymd(2018, 11, 4).and_hms(4,59,59).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mst_offset, 4), 6);
        let now = mst.ymd(2018, 11, 4).and_hms(5,0,0).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mst_offset, 4), 6);
        let now = mst.ymd(2018, 11, 4).and_hms(6,0,0).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mst_offset, 4), 6);
        // days elapsed should remain 6 untl rollover on 5 Nov
        let now = mst.ymd(2018, 11, 4).and_hms(23,59,59).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mst_offset, 4), 6);
        let now = mst.ymd(2018, 11, 5).and_hms(0,0,0).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mst_offset, 4), 6);
        let now = mst.ymd(2018, 11, 5).and_hms(1,0,0).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mst_offset, 4), 6);
        let now = mst.ymd(2018, 11, 5).and_hms(2,0,0).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mst_offset, 4), 6);
        let now = mst.ymd(2018, 11, 5).and_hms(3,0,0).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mst_offset, 4), 6);
        let now = mst.ymd(2018, 11, 5).and_hms(3,59,59).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mst_offset, 4), 6);
        let now = mst.ymd(2018, 11, 5).and_hms(4,0,0).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mst_offset, 4), 7);
        // Similarly on subsequent days
        let now = mst.ymd(2018, 11, 5).and_hms(23,59,59).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mst_offset, 4), 7);
        let now = mst.ymd(2018, 11, 6).and_hms(0,0,0).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mst_offset, 4), 7);
        let now = mst.ymd(2018, 11, 6).and_hms(1,0,0).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mst_offset, 4), 7);
        let now = mst.ymd(2018, 11, 6).and_hms(2,0,0).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mst_offset, 4), 7);
        let now = mst.ymd(2018, 11, 6).and_hms(3,0,0).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mst_offset, 4), 7);
        let now = mst.ymd(2018, 11, 6).and_hms(3,59,59).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mst_offset, 4), 7);
        let now = mst.ymd(2018, 11, 6).and_hms(4,0,0).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mst_offset, 4), 8);
        let now = mst.ymd(2018, 11, 6).and_hms(23,59,59).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mst_offset, 4), 8);
        let now = mst.ymd(2018, 11, 7).and_hms(0,0,0).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mst_offset, 4), 8);
        let now = mst.ymd(2018, 11, 7).and_hms(1,0,0).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mst_offset, 4), 8);
        let now = mst.ymd(2018, 11, 7).and_hms(2,0,0).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mst_offset, 4), 8);

        //
        //
        // Test transition from MST to MDT
        //
        println!();
        println!("MST to MDT");
        let crt = mst.ymd(2019, 3, 3).and_hms(3, 0, 1).timestamp();
        let offset = mst.utc_minus_local() / 60;
        let now = mst.ymd(2019, 3, 3).and_hms(4,0,0).timestamp();
        assert_eq!(elap(crt, now, mst_offset, mst_offset, 4), 0);
        let now = mst.ymd(2019, 3, 4).and_hms(4,0,0).timestamp();
        assert_eq!(elap(crt, now, mst_offset, mst_offset, 4), 1);
        let now = mst.ymd(2019, 3, 5).and_hms(4,0,0).timestamp();
        assert_eq!(elap(crt, now, mst_offset, mst_offset, 4), 2);
        let now = mst.ymd(2019, 3, 6).and_hms(4,0,0).timestamp();
        assert_eq!(elap(crt, now, mst_offset, mst_offset, 4), 3);
        let now = mst.ymd(2019, 3, 9).and_hms(4,0,0).timestamp();
        assert_eq!(elap(crt, now, mst_offset, mst_offset, 4), 6);
        // On 10 Mar, switch to MDT at 2am
        // Until the switch, days elapsed should be 5
        let now = mst.ymd(2019, 3, 10).and_hms(0,0,0).timestamp();
        assert_eq!(elap(crt, now, mst_offset, mst_offset, 4), 6);
        let now = mst.ymd(2019, 3, 10).and_hms(1,0,0).timestamp();
        assert_eq!(elap(crt, now, mst_offset, mst_offset, 4), 6);
        println!("2am MST");
        let now = mst.ymd(2019, 3, 10).and_hms(2,0,0).timestamp();
        assert_eq!(elap(crt, now, mst_offset, mst_offset, 4), 6);
        let offset = mdt.utc_minus_local() / 60;
        println!("3am MDT");
        let now = mdt.ymd(2019, 3, 10).and_hms(3,0,0).timestamp();
        assert_eq!(elap(crt, now, mst_offset, mdt_offset, 4), 6);
        let now = mdt.ymd(2019, 3, 10).and_hms(3,59,59).timestamp();
        assert_eq!(elap(crt, now, mst_offset, mdt_offset, 4), 6);
        let now = mdt.ymd(2019, 3, 10).and_hms(4,0,0).timestamp();
        assert_eq!(elap(crt, now, mst_offset, mdt_offset, 4), 7);
        let now = mdt.ymd(2019, 3, 10).and_hms(5,0,0).timestamp();
        assert_eq!(elap(crt, now, mst_offset, mdt_offset, 4), 7);
        let now = mdt.ymd(2019, 3, 11).and_hms(4,0,0).timestamp();
        assert_eq!(elap(crt, now, mst_offset, mdt_offset, 4), 8);
        let now = mdt.ymd(2019, 3, 12).and_hms(4,0,0).timestamp();
        assert_eq!(elap(crt, now, mst_offset, mdt_offset, 4), 9);

        // Now test around the transition MDT to MST
        let now = mdt.ymd(2019, 11, 1).and_hms(4,0,0).timestamp();
        assert_eq!(elap(crt, now, mst_offset, mdt_offset, 4), 243);
        let now = mdt.ymd(2019, 11, 2).and_hms(4,0,0).timestamp();
        assert_eq!(elap(crt, now, mst_offset, mdt_offset, 4), 244);
        println!("2am MDT");
        let now = mdt.ymd(2019, 11, 3).and_hms(2,0,0).timestamp();
        assert_eq!(elap(crt, now, mst_offset, mdt_offset, 4), 244);
        let offset = mst.utc_minus_local() / 60;
        println!("1am MST");
        let now = mst.ymd(2019, 11, 3).and_hms(1,0,0).timestamp();
        assert_eq!(elap(crt, now, mst_offset, mst_offset, 4), 244);
        let now = mst.ymd(2019, 11, 3).and_hms(2,0,0).timestamp();
        assert_eq!(elap(crt, now, mst_offset, mst_offset, 4), 244);
        let now = mst.ymd(2019, 11, 3).and_hms(3,0,0).timestamp();
        assert_eq!(elap(crt, now, mst_offset, mst_offset, 4), 244);
        let now = mst.ymd(2019, 11, 3).and_hms(4,0,0).timestamp();
        assert_eq!(elap(crt, now, mst_offset, mst_offset, 4), 245);
        let now = mst.ymd(2019, 11, 3).and_hms(5,0,0).timestamp();
        assert_eq!(elap(crt, now, mst_offset, mst_offset, 4), 245);
        let now = mst.ymd(2019, 11, 4).and_hms(4,0,0).timestamp();
        assert_eq!(elap(crt, now, mst_offset, mst_offset, 4), 246);
        let now = mst.ymd(2019, 11, 5).and_hms(4,0,0).timestamp();
        assert_eq!(elap(crt, now, mst_offset, mst_offset, 4), 247);


        //
        //
        // Test transition from MST to MDT
        //
        println!();
        let crt = mst.ymd(2019, 3, 3).and_hms(4, 0, 0).timestamp();
        let offset = mst.utc_minus_local() / 60;
        let now = mst.ymd(2019, 3, 3).and_hms(4,0,0).timestamp();
        assert_eq!(elap(crt, now, mst_offset, mst_offset, 4), 0);
        let now = mst.ymd(2019, 3, 4).and_hms(4,0,0).timestamp();
        assert_eq!(elap(crt, now, mst_offset, mst_offset, 4), 1);
        let now = mst.ymd(2019, 3, 5).and_hms(4,0,0).timestamp();
        assert_eq!(elap(crt, now, mst_offset, mst_offset, 4), 2);
        let now = mst.ymd(2019, 3, 6).and_hms(4,0,0).timestamp();
        assert_eq!(elap(crt, now, mst_offset, mst_offset, 4), 3);
        let now = mst.ymd(2019, 3, 9).and_hms(4,0,0).timestamp();
        assert_eq!(elap(crt, now, mst_offset, mst_offset, 4), 6);

        // On 10 Mar, switch to MDT at 2am
        // Until the switch, days elapsed should be 5
        let now = mst.ymd(2019, 3, 10).and_hms(0,0,0).timestamp();
        assert_eq!(elap(crt, now, mst_offset, mst_offset, 4), 6);
        let now = mst.ymd(2019, 3, 10).and_hms(1,0,0).timestamp();
        assert_eq!(elap(crt, now, mst_offset, mst_offset, 4), 6);
        let now = mst.ymd(2019, 3, 10).and_hms(2,0,0).timestamp();
        assert_eq!(elap(crt, now, mst_offset, mst_offset, 4), 6);
        let offset = mdt.utc_minus_local() / 60;
        let now = mdt.ymd(2019, 3, 10).and_hms(3,0,0).timestamp();
        assert_eq!(elap(crt, now, mst_offset, mdt_offset, 4), 6);
        let now = mdt.ymd(2019, 3, 10).and_hms(3,59,59).timestamp();
        assert_eq!(elap(crt, now, mst_offset, mdt_offset, 4), 6);
        let now = mdt.ymd(2019, 3, 10).and_hms(4,0,0).timestamp();
        assert_eq!(elap(crt, now, mst_offset, mdt_offset, 4), 7);
        let now = mdt.ymd(2019, 3, 10).and_hms(5,0,0).timestamp();
        assert_eq!(elap(crt, now, mst_offset, mdt_offset, 4), 7);
        let now = mdt.ymd(2019, 3, 11).and_hms(4,0,0).timestamp();
        assert_eq!(elap(crt, now, mst_offset, mdt_offset, 4), 8);
        let now = mdt.ymd(2019, 3, 12).and_hms(4,0,0).timestamp();
        assert_eq!(elap(crt, now, mst_offset, mdt_offset, 4), 9);


        //
        //
        // Test transition from MST to MDT
        //
        println!();
        let crt = mst.ymd(2019, 3, 3).and_hms(4, 0, 1).timestamp();
        let offset = mst.utc_minus_local() / 60;
        let now = mst.ymd(2019, 3, 3).and_hms(4,0,0).timestamp();
        assert_eq!(elap(crt, now, mst_offset, mst_offset, 4), 0);
        let now = mst.ymd(2019, 3, 4).and_hms(4,0,0).timestamp();
        assert_eq!(elap(crt, now, mst_offset, mst_offset, 4), 1);
        let now = mst.ymd(2019, 3, 5).and_hms(4,0,0).timestamp();
        assert_eq!(elap(crt, now, mst_offset, mst_offset, 4), 2);
        let now = mst.ymd(2019, 3, 6).and_hms(4,0,0).timestamp();
        assert_eq!(elap(crt, now, mst_offset, mst_offset, 4), 3);
        let now = mst.ymd(2019, 3, 9).and_hms(4,0,0).timestamp();
        assert_eq!(elap(crt, now, mst_offset, mst_offset, 4), 6);

        // On 10 Mar, switch to MDT at 2am
        // Until the switch, days elapsed should be 5
        let now = mst.ymd(2019, 3, 10).and_hms(0,0,0).timestamp();
        assert_eq!(elap(crt, now, mst_offset, mst_offset, 4), 6);
        let now = mst.ymd(2019, 3, 10).and_hms(1,0,0).timestamp();
        assert_eq!(elap(crt, now, mst_offset, mst_offset, 4), 6);
        let now = mst.ymd(2019, 3, 10).and_hms(2,0,0).timestamp();
        assert_eq!(elap(crt, now, mst_offset, mst_offset, 4), 6);
        let offset = mdt.utc_minus_local() / 60;
        let now = mdt.ymd(2019, 3, 10).and_hms(3,0,0).timestamp();
        assert_eq!(elap(crt, now, mst_offset, mdt_offset, 4), 6);
        let now = mdt.ymd(2019, 3, 10).and_hms(3,59,59).timestamp();
        assert_eq!(elap(crt, now, mst_offset, mdt_offset, 4), 6);
        let now = mdt.ymd(2019, 3, 10).and_hms(4,0,0).timestamp();
        assert_eq!(elap(crt, now, mst_offset, mdt_offset, 4), 7);
        let now = mdt.ymd(2019, 3, 10).and_hms(5,0,0).timestamp();
        assert_eq!(elap(crt, now, mst_offset, mdt_offset, 4), 7);
        let now = mdt.ymd(2019, 3, 11).and_hms(4,0,0).timestamp();
        assert_eq!(elap(crt, now, mst_offset, mdt_offset, 4), 8);
        let now = mdt.ymd(2019, 3, 12).and_hms(4,0,0).timestamp();
        assert_eq!(elap(crt, now, mst_offset, mdt_offset, 4), 9);



        // If creation is less than one hour before rollover the
        // remainder of the days calculation is less than 3600.
        // This will be problematic if the collection is created
        // during standard time as the subsequent transition to
        // DST makes the day an hour shorter. On the day of the
        // transition to DST, days will decrement at the time of
        // transition to DST, causing a 1 day offset for the 
        // duration of DST. Then, on the subsequent transition 
        // from DST it will increment at the transition.
        println!();
        println!("remainder 0 to 3600");
        let crt = mst.ymd(2019, 3, 3).and_hms(3, 0, 1).timestamp();
        let offset = mst.utc_minus_local() / 60;
        let now = mst.ymd(2019, 3, 4).and_hms(4,0,0).timestamp();
        assert_eq!(elap(crt, now, mst_offset, mst_offset, 4), 1);
        let now = mst.ymd(2019, 3, 5).and_hms(4,0,0).timestamp();
        assert_eq!(elap(crt, now, mst_offset, mst_offset, 4), 2);
        // At the transition to DST the day is an hour shorter
        // causing days to not increment on the day of transition.
        let now = mst.ymd(2019, 3, 9).and_hms(4,0,0).timestamp();
        assert_eq!(elap(crt, now, mst_offset, mst_offset, 4), 6);
        let now = mst.ymd(2019, 3, 10).and_hms(2,0,0).timestamp();
        assert_eq!(elap(crt, now, mst_offset, mst_offset, 4), 6);
        let offset = mdt.utc_minus_local() / 60;
        let now = mdt.ymd(2019, 3, 10).and_hms(3,0,0).timestamp();
        assert_eq!(elap(crt, now, mst_offset, mdt_offset, 4), 6);
        let now = mdt.ymd(2019, 3, 10).and_hms(4,0,0).timestamp();
        assert_eq!(elap(crt, now, mst_offset, mdt_offset, 4), 7);
        let now = mdt.ymd(2019, 3, 11).and_hms(4,0,0).timestamp();
        assert_eq!(elap(crt, now, mst_offset, mdt_offset, 4), 8);

        let now = mdt.ymd(2019, 11, 1).and_hms(4,0,0).timestamp();
        assert_eq!(elap(crt, now, mst_offset, mdt_offset, 4), 243);
        let now = mdt.ymd(2019, 11, 2).and_hms(4,0,0).timestamp();
        assert_eq!(elap(crt, now, mst_offset, mdt_offset, 4), 244);
        let now = mdt.ymd(2019, 11, 3).and_hms(2,0,0).timestamp();
        assert_eq!(elap(crt, now, mst_offset, mdt_offset, 4), 244);
        let offset = mst.utc_minus_local() / 60;
        let now = mst.ymd(2019, 11, 3).and_hms(1,0,0).timestamp();
        assert_eq!(elap(crt, now, mst_offset, mst_offset, 4), 244);
        let now = mst.ymd(2019, 11, 3).and_hms(2,0,0).timestamp();
        assert_eq!(elap(crt, now, mst_offset, mst_offset, 4), 244);
        let now = mst.ymd(2019, 11, 3).and_hms(3,0,0).timestamp();
        assert_eq!(elap(crt, now, mst_offset, mst_offset, 4), 244);
        let now = mst.ymd(2019, 11, 3).and_hms(4,0,0).timestamp();
        assert_eq!(elap(crt, now, mst_offset, mst_offset, 4), 245);
        let now = mst.ymd(2019, 11, 4).and_hms(4,0,0).timestamp();
        assert_eq!(elap(crt, now, mst_offset, mst_offset, 4), 246);

        // If creation is less than one hour after rollover the
        // remainder of the days calculation is more than 82800.
        // This will be problematic if the collection is created
        // during DST as the subsequent transition to standard
        // time makes the day an hour longer. On the day of the
        // transition to standard time, days will increment at
        // the time of transition to standard time, causing a 1
        // day offset for the duration of standard time. Then,
        // on the subsequent transition back to DST it will
        // decrement at the transition.
        println!();
        println!("remainder 0 to 3600");
        let crt = mdt.ymd(2018, 11, 1).and_hms(4, 59, 59).timestamp();
        let offset = mdt.utc_minus_local() / 60;
        let now = mdt.ymd(2018, 11, 2).and_hms(4,0,0).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mdt_offset, 4), 1);
        let now = mdt.ymd(2018, 11, 3).and_hms(4,0,0).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mdt_offset, 4), 2);

        // MDT to MST Sun 4 Nov 2018 at 2am
        let now = mdt.ymd(2018, 11, 4).and_hms(2,0,0).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mdt_offset, 4), 2);
        let offset = mst.utc_minus_local() / 60;
        let now = mst.ymd(2018, 11, 4).and_hms(1,0,0).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mst_offset, 4), 2);
        let now = mst.ymd(2018, 11, 4).and_hms(4,0,0).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mst_offset, 4), 3);
        let now = mst.ymd(2018, 11, 5).and_hms(4,0,0).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mst_offset, 4), 4);
        //...
        let now = mst.ymd(2019, 3, 9).and_hms(4,0,0).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mst_offset, 4), 128);

        // MST to MDT Sun, 10 Mar 2019 at 2am
        let now = mst.ymd(2019, 3, 10).and_hms(2,0,0).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mst_offset, 4), 128);
        let offset = mdt.utc_minus_local() / 60;
        let now = mdt.ymd(2019, 3, 10).and_hms(3,0,0).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mdt_offset, 4), 128);
        let now = mdt.ymd(2019, 3, 10).and_hms(2,0,0).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mdt_offset, 4), 128);
        let now = mdt.ymd(2019, 3, 10).and_hms(3,0,0).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mdt_offset, 4), 128);
        let now = mdt.ymd(2019, 3, 10).and_hms(4,0,0).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mdt_offset, 4), 129);

        let now = mdt.ymd(2019, 3, 11).and_hms(4,0,0).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mdt_offset, 4), 130);
        let now = mdt.ymd(2019, 3, 12).and_hms(4,0,0).timestamp();
        assert_eq!(elap(crt, now, mdt_offset, mdt_offset, 4), 131);


        // Test Duration::days(1) at MST/MDT transitions
        // let next_day_at = (rollover_today_datetime + Duration::days(1)).timestamp();
        // First, a day that is not a MST/MDT transition
        let start = mst.ymd(2019, 3, 3).and_hms(2,0,0);
        let next_day_at = (start + Duration::days(1)).timestamp();
        let end = start + Duration::days(1);
        let start_ts = start.timestamp();
        let end_ts = end.timestamp();
        assert_eq!((end_ts - start_ts), 86400);

        // MST to MDT Sun, 10 Mar 2019 at 2am
        println!();
        let mdt = FixedOffset::west(6 * 60 * 60);
        let mst = FixedOffset::west(7 * 60 * 60);
        let start = mst.ymd(2019, 3, 10).and_hms(0,0,0);
        println!("start: {}", start);
        let end = mdt.ymd(2019, 3, 11).and_hms(0,0,0);
        println!("end: {}", end);
        let elapsed_seconds = end.timestamp() - start.timestamp();
        println!("elapsed_seconds: {}", elapsed_seconds);
        let end2 = start + Duration::days(1);
        println!("end2: {}", end2);
        let elapsed_seconds2 = end2.timestamp() - start.timestamp();
        println!("elapsed_seconds2: {}", elapsed_seconds2);
        println!();


        println!();
        let start = Local.ymd(2019, 3, 10).and_hms(0,0,0);
        println!("start: {}", start);
        let end = Local.ymd(2019, 3, 11).and_hms(0,0,0);
        println!("end: {}", end);
        let elapsed_seconds = end.timestamp() - start.timestamp();
        println!("elapsed_seconds: {}", elapsed_seconds);
        let end2 = start + Duration::days(1);
        println!("end2: {}", end2);
        let elapsed_seconds2 = end2.timestamp() - start.timestamp();
        println!("elapsed_seconds2: {}", elapsed_seconds2);
        println!();


        println!();
        let start = Local.ymd(2019, 3, 10).and_hms(0,0,0);
        println!("start: {}", start);
        let end = Local.ymd(2019, 3, 11).and_hms(0,0,0);
        println!("end: {}", end);
        let elapsed_seconds = end.timestamp() - start.timestamp();
        println!("elapsed_seconds: {}", elapsed_seconds);
        let end2 = (start.date() + Duration::days(1)).and_hms(start.hour(), 0, 0);
        println!("end2: {}", end2);
        let elapsed_seconds2 = end2.timestamp() - start.timestamp();
        println!("elapsed_seconds2: {}", elapsed_seconds2);
        println!();


        println!();
        let mdt = FixedOffset::west(6 * 60 * 60);
        let mst = FixedOffset::west(7 * 60 * 60);
        let start = mst.ymd(2019, 3, 10).and_hms(0,0,0);
        println!("start: {}", start);
        let end = mdt.ymd(2019, 3, 11).and_hms(0,0,0);
        println!("end: {}", end);
        let elapsed_seconds = end.timestamp() - start.timestamp();
        println!("elapsed_seconds: {}", elapsed_seconds);
        let end2 = (start.date() + Duration::days(1)).and_hms(start.hour(), 0, 0);
        println!("end2: {}", end2);
        let elapsed_seconds2 = end2.timestamp() - start.timestamp();
        println!("elapsed_seconds2: {}", elapsed_seconds2);
        println!();




        // Find initial conditions with remainder of days calculation
        // in the range 0 to 3600. These will be problematic when a
        // transition to/from DST makes a day an hour shorter.
        //
        // Remainder of the days calculation is in the range 0 to 3600
        // if creation time is in the hour before rollover time. These
        // cases will be problematic when a transition to DST make the
        // day one hour shorter.
        println!();
        println!("remainder 0 to 3600");
        let crt = mst.ymd(2019, 3, 3).and_hms(4, 0, 0).timestamp();
        let offset = mst.utc_minus_local() / 60;
        let now = mst.ymd(2019, 3, 4).and_hms(4,0,0).timestamp();
        // assert_eq!(elap(crt, now, mst_offset, mst_offset, 4), 0);
        elap(crt, now, mst_offset, mst_offset, 4);
        let crt = mst.ymd(2019, 3, 3).and_hms(3, 0, 0).timestamp();
        let offset = mst.utc_minus_local() / 60;
        let now = mst.ymd(2019, 3, 4).and_hms(4,0,0).timestamp();
        // assert_eq!(elap(crt, now, mst_offset, mst_offset, 4), 0);
        elap(crt, now, mst_offset, mst_offset, 4);


        // Find initial conditions with remainder of days calculation
        // in the range 82800 to 86399. These will be problematic when
        // a transition to/from DST makes a day an hour longer.
        println!();
        println!("remainder 82800 to 86399");
        let crt = mst.ymd(2019, 3, 3).and_hms(4, 0, 1).timestamp();
        let offset = mst.utc_minus_local() / 60;
        let now = mst.ymd(2019, 3, 4).and_hms(4,0,0).timestamp();
        // assert_eq!(elap(crt, now, mst_offset, mst_offset, 4), 0);
        elap(crt, now, mst_offset, mst_offset, 4);
        let crt = mst.ymd(2019, 3, 3).and_hms(5, 0, 0).timestamp();
        let offset = mst.utc_minus_local() / 60;
        let now = mst.ymd(2019, 3, 4).and_hms(4,0,0).timestamp();
        // assert_eq!(elap(crt, now, mst_offset, mst_offset, 4), 0);
        elap(crt, now, mst_offset, mst_offset, 4);

        // sure to fail
        assert_eq!(111, 222);

    }
}
