use chrono::{DateTime, TimeZone};
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
    let dt = DateTime::parse_from_rfc3339(date_str)
        .map_err(|e| format!("invalid date-time: {}", e))
        .unwrap();
    set_timestamp_datetime(&dt);
}

pub fn set_timestamp_datetime<T: TimeZone>(timestamp: &DateTime<T>) {
    set_timestamp(timestamp.timestamp());
}