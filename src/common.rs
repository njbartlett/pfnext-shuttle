use std::fmt::Display;

use chrono::{DateTime, FixedOffset, NaiveDate};
use result::OptionResultExt;
use rocket::http::Status;
use crate::apierror::ApiError;

pub fn parse_opt_date(str: Option<String>) -> Result<Option<DateTime<FixedOffset>>, ApiError> {
    str.map(|s| DateTime::parse_from_rfc3339(&s))
        .invert()
        .map_err(|e| ApiError::new(Status::UnprocessableEntity, e.to_string()))
}

pub fn parse_opt_naive_date(str: Option<String>, fmt: &str)  -> Result<Option<NaiveDate>, ApiError> {
    str.map(|s| NaiveDate::parse_from_str(&s, fmt))
        .invert()
        .map_err(|e| ApiError::new(Status::UnprocessableEntity, e.to_string()))
}

pub fn to_internal_server_err<D: Display>(err: D) -> ApiError {
    warn!("Internal Server Error: {err}");
    ApiError::new(Status::InternalServerError, err.to_string())
}