use chrono::DateTime;
use std::cell::Cell;

thread_local! {
    static TIMESTAMP: Cell<i64> = const { Cell::new(0) };
}

pub struct Utc;

impl Utc {
    pub fn now() -> DateTime<chrono::Utc> {
        TIMESTAMP.with(|timestamp| DateTime::<chrono::Utc>::from_timestamp(
            timestamp.get(),
            0,
        )).expect("a valid timestamp set")
    }
}

pub fn set_timestamp(timestamp: i64) {
    TIMESTAMP.with(|ts| ts.set(timestamp));
}

pub fn set_timestamp_rfc3339(date_str: &str) {
    set_timestamp(DateTime::parse_from_rfc3339(date_str).expect("invalid date-time").timestamp());
}