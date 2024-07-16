use std::cmp::Ordering;

use chrono::{DateTime, FixedOffset, NaiveDate, Utc};

use itertools::Itertools;

use rocket::form::{FromForm, FromFormField, ValueField};
use rocket::form::prelude::ErrorKind::Custom;
use rocket::http::Status;
use rocket::response::status::BadRequest;
use rocket::serde::Deserialize;
use rocket::serde::json::Json;
use rocket::State;
use rocket::time::format_description::parse;

use serde::Serialize;
use shuttle_runtime::CustomError;
use sqlx::{FromRow, query, query_as, QueryBuilder};
use sqlx::query::Query;

use crate::AppState;
use crate::claims::Claims;

#[derive(Serialize, FromRow, Clone, Debug)]
struct Session {
    id: i64,
    datetime: DateTime<Utc>,
    duration_mins: i32,
    session_type: String,
    location: String,
    trainer: String,
    booked: bool,
    booking_count: i64
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

async fn fetch_sessions(state: &State<AppState>, claim: Claims, from_str: Option<String>, to_str: Option<String>) -> Result<Vec<Session>, BadRequest<String>> {
    let mut qb = QueryBuilder::new("SELECT s.id, s.datetime, s.duration_mins, t.name as session_type, loc.name as location, trainer.name as trainer, \
        CASE WHEN EXISTS (SELECT 1 FROM booking WHERE booking.session_id = s.id AND booking.person_id = ");
    qb.push_bind(claim.uid);
    qb.push(") THEN true ELSE false END AS booked, \
        (SELECT COUNT(*) FROM booking WHERE booking.session_id = s.id) AS booking_count \
        FROM session as s, session_type as t, location as loc, person as trainer \
        WHERE s.session_type = t.id AND s.location = loc.id AND s.trainer = trainer.id");

    if let Some(from) = parse_opt_date(from_str)? {
        qb.push(" AND s.datetime >= ");
        qb.push_bind(from);
    }
    if let Some(to) = parse_opt_date(to_str)? {
        qb.push(" AND s.datetime <= ");
        qb.push_bind(to);
    }
    let sessions = qb.build_query_as()
        .fetch_all(&state.pool)
        .await
        .map_err(|e| BadRequest(e.to_string()))?;
    Ok(sessions)
}

#[get("/sessions/list?<from>&<to>")]
pub async fn list_sessions(state: &State<AppState>, claim: Claims, from: Option<String>, to: Option<String>) -> Result<Json<Vec<Session>>, BadRequest<String>> {
    let sessions = fetch_sessions(state, claim, from, to).await?;
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

#[get("/sessions/list_by_date?<from>&<to>")]
pub async fn list_sessions_by_date(state: &State<AppState>, claim: Claims, from: Option<String>, to: Option<String>) -> Result<Json<Vec<SessionDate>>, BadRequest<String>> {
    let session_dates: Vec<SessionDate> = fetch_sessions(state, claim, from, to).await?
        .into_iter()
        .into_group_map_by(|s| s.datetime.naive_local().date())
        .into_iter()
        .map(|(k, v)| SessionDate { date: k, sessions: v })
        .sorted()
        .collect();
    Ok(Json(session_dates))
}

#[derive(Deserialize, Debug)]
struct SessionBookingRequest {
    session_id: i64
}

#[derive(Serialize, FromRow, Debug)]
struct SessionBookingResponse {
    person_id: i64,
    session_id: i64
}

#[post("/bookings", data="<session>")]
pub async fn book_session(state: &State<AppState>, claim: Claims, session: Json<SessionBookingRequest>) -> Result<Json<Vec<SessionBookingResponse>>, BadRequest<String>> {
    let bookings_created = query_as("INSERT INTO booking (person_id, session_id) VALUES ($1, $2) ON CONFLICT DO NOTHING RETURNING person_id, session_id")
        .bind(claim.uid)
        .bind(session.session_id)
        .fetch_all(&state.pool)
        .await
        .map_err(|e| BadRequest(e.to_string()))?;
    Ok(Json(bookings_created))
}

#[delete("/bookings", data="<session>")]
pub async fn cancel_booking(state: &State<AppState>, claim: Claims, session: Json<SessionBookingRequest>) -> Result<Json<Vec<SessionBookingResponse>>, BadRequest<String>> {
    let bookings_deleted = query_as("DELETE FROM booking WHERE person_id = $1 AND session_id = $2 RETURNING person_id, session_id")
        .bind(claim.uid)
        .bind(session.session_id)
        .fetch_all(&state.pool)
        .await
        .map_err(|e| BadRequest(e.to_string()))?;
    Ok(Json(bookings_deleted))
}