use crate::error::{Error, Result};
use chrono::prelude::*;

pub fn now_utc() -> DateTime<Utc> {
    Utc::now()
}

pub fn format_time(time: DateTime<Utc>) -> String {
    time.to_rfc3339()
}

pub fn now_utc_plus_sec(sec: i64) -> String {
    let time = Utc::now() + chrono::Duration::seconds(sec);
    time.to_rfc3339()
}
pub fn now_utc_plus_days(days: i64) -> String {
    let time = Utc::now() + chrono::Duration::days(days);
    time.to_rfc3339()
}

pub fn utc_plus_days(timestamp: NaiveDateTime, days: i64) -> Option<NaiveDateTime> {
    timestamp.checked_add_signed(chrono::Duration::days(days))
}

pub fn parse_time(time: &str) -> Result<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(time)
        .map(|t| t.with_timezone(&Utc))
        .map_err(|_| Error::FailToDateParse(time.to_string()))
}

// region: Unit Test
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_now_utc() {
        let time = now_utc();
        let now = Utc::now();
        assert!(time.timestamp() <= now.timestamp());
    }

    #[test]
    fn test_format_time() {
        let time = Utc.with_ymd_and_hms(2020, 1, 1, 0, 0, 0);
        let formatted = format_time(time.unwrap());
        assert_eq!(formatted, "2020-01-01T00:00:00+00:00");
    }

    #[test]
    fn test_now_plus_days() {
        let time = now_utc_plus_days(12);
        let now = Utc::now();
        println!("Now: {:?}, Time: {:?}", now, time);
    }

    #[test]
    fn test_parse_time() {
        let time = "2020-01-01T00:00:00+00:00";
        let parsed = parse_time(time).unwrap();
        let compare = Utc.with_ymd_and_hms(2020, 1, 1, 0, 0, 0);
        assert_eq!(parsed, compare.unwrap());
    }
}

// endregion: Unit Test
