use std::cmp::Ordering;

use chrono::{DateTime, FixedOffset, NaiveDate, Utc};

use itertools::Itertools;

use rocket::form::{FromForm, FromFormField, ValueField};
use rocket::form::prelude::ErrorKind::Custom;
use rocket::http::Status;
use rocket::response::status::BadRequest;
use rocket::serde::json::Json;
use rocket::State;
use rocket::time::format_description::parse;

use serde::Serialize;
use shuttle_runtime::CustomError;
use sqlx::{FromRow, query, QueryBuilder};
use sqlx::query::Query;

use crate::AppState;

#[derive(Serialize, FromRow, Clone, Debug)]
struct Session {
    id: i64,
    datetime: DateTime<Utc>,
    duration_mins: i32,
    session_type: String,
    location: String
}

fn parse_opt_date(str: Option<String>) -> Result<Option<DateTime<FixedOffset>>, BadRequest<String>> {
    if str.is_none() {
        return Ok(None);
    }
    let parsed = DateTime::parse_from_rfc3339(str.as_ref().unwrap());
    println!("Parsed input {:?} to {:?}", &str, parsed);
        //.map_err(|e| BadRequest(e.to_string()))?;
    Ok(Some(parsed.map_err(|e| BadRequest(e.to_string()))?))
}

async fn fetch_sessions(state: &State<AppState>, from_str: Option<String>, to_str: Option<String>) -> Result<Vec<Session>, BadRequest<String>> {
    let mut query = QueryBuilder::new("SELECT s.id, s.datetime, s.duration_mins, t.name as session_type, l.name as location \
        FROM session as s, session_type as t, location as l \
        WHERE s.session_type = t.id AND s.location = l.id");
    if let Some(from) = parse_opt_date(from_str)? {
        query.push(" AND s.datetime >= ");
        query.push_bind(from);
    }
    if let Some(to) = parse_opt_date(to_str)? {
        query.push(" AND s.datetime <= ");
        query.push_bind(to);
    }
    let sessions = query.build_query_as()
        .fetch_all(&state.pool)
        .await
        .map_err(|e| BadRequest(e.to_string()))?;
    Ok(sessions)
}

#[get("/sessions?<from>&<to>")]
pub async fn list_sessions(state: &State<AppState>, from: Option<String>, to: Option<String>) -> Result<Json<Vec<Session>>, BadRequest<String>> {
    let sessions = fetch_sessions(state, from, to).await?;
    Ok(Json(sessions))
}

#[derive(Serialize, Debug)]
struct SessionDate {
    date: chrono::NaiveDate,
    sessions: Vec<Session>
}

impl PartialEq<Self> for SessionDate {
    fn eq(&self, other: &Self) -> bool {
        return false;
    }
}

impl PartialOrd for SessionDate {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        return Some(self.date.cmp(&other.date));
    }
}

impl Eq for SessionDate {}

impl Ord for SessionDate {
    fn cmp(&self, other: &Self) -> Ordering {
        return self.date.cmp(&other.date);
    }
}

#[get("/sessions_by_date?<from>&<to>")]
pub async fn list_sessions_by_date(state: &State<AppState>, from: Option<String>, to: Option<String>) -> Result<Json<Vec<SessionDate>>, BadRequest<String>> {
    let session_dates: Vec<SessionDate> = fetch_sessions(state, from, to).await?
        .into_iter()
        .into_group_map_by(|s| s.datetime.naive_local().date())
        .into_iter()
        .map(|(k, v)| SessionDate { date: k, sessions: v })
        .sorted()
        .collect();
    Ok(Json(session_dates))
}