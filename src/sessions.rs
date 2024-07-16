use std::cmp::Ordering;

use chrono::{DateTime, FixedOffset, Utc};
use itertools::Itertools;
use rocket::http::Status;
use rocket::response::status::Custom;
use rocket::serde::Deserialize;
use rocket::serde::json::Json;
use rocket::State;
use serde::Serialize;
use sqlx::{FromRow, query_as, QueryBuilder};

use crate::AppState;
use crate::claims::Claims;

#[derive(Serialize, FromRow, Clone, Debug)]
pub struct Session {
    id: i64,
    datetime: DateTime<Utc>,
    duration_mins: i32,
    session_type: String,
    location: String,
    trainer: String,
    booked: bool,
    booking_count: i64
}

fn parse_opt_date(str: Option<String>) -> Result<Option<DateTime<FixedOffset>>, Custom<String>> {
    if str.is_none() {
        return Ok(None);
    }
    let parsed = DateTime::parse_from_rfc3339(str.as_ref().unwrap());
    println!("Parsed input {:?} to {:?}", &str, parsed);
        //.map_err(|e| BadRequest(e.to_string()))?;
    Ok(Some(parsed.map_err(|e| Custom(Status::UnprocessableEntity, e.to_string()))?))
}

async fn fetch_sessions(state: &State<AppState>, claim: Claims, from_str: Option<String>, to_str: Option<String>) -> Result<Vec<Session>, Custom<String>> {
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
        .map_err(|e| Custom(Status::InternalServerError, e.to_string()))?;
    Ok(sessions)
}

#[get("/sessions/list?<from>&<to>")]
pub async fn list_sessions(state: &State<AppState>, claim: Claims, from: Option<String>, to: Option<String>) -> Result<Json<Vec<Session>>, Custom<String>> {
    let sessions = fetch_sessions(state, claim, from, to).await?;
    Ok(Json(sessions))
}

#[derive(Serialize, Debug)]
pub struct SessionDate {
    date: chrono::NaiveDate,
    sessions: Vec<Session>
}

impl PartialEq<Self> for SessionDate {
    fn eq(&self, _other: &Self) -> bool {
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
pub async fn list_sessions_by_date(state: &State<AppState>, claim: Claims, from: Option<String>, to: Option<String>) -> Result<Json<Vec<SessionDate>>, Custom<String>> {
    let session_dates: Vec<SessionDate> = fetch_sessions(state, claim, from, to).await?
        .into_iter()
        .into_group_map_by(|s| s.datetime.naive_local().date())
        .into_iter()
        .map(|(k, v)| SessionDate { date: k, sessions: v })
        .sorted()
        .collect();
    Ok(Json(session_dates))
}

#[derive(Serialize, Deserialize, FromRow, Debug)]
pub struct SessionBooking {
    person_id: i64,
    session_id: i64
}

#[post("/bookings", data="<session>")]
pub async fn book_session(state: &State<AppState>, claim: Claims, session: Json<SessionBooking>) -> Result<Json<Vec<SessionBooking>>, Custom<String>> {
    if session.person_id != claim.uid && !is_admin(&claim) {
        return Err(Custom(Status::Forbidden, "not allowed to create bookings for other users".to_string()));
    }
    let bookings_created = query_as("INSERT INTO booking (person_id, session_id) VALUES ($1, $2) ON CONFLICT DO NOTHING RETURNING person_id, session_id")
        .bind(claim.uid)
        .bind(session.session_id)
        .fetch_all(&state.pool)
        .await
        .map_err(|e| Custom(Status::InternalServerError, e.to_string()))?;
    Ok(Json(bookings_created))
}

#[delete("/bookings", data="<booking>")]
pub async fn cancel_booking(state: &State<AppState>, claim: Claims, booking: Json<SessionBooking>) -> Result<Json<Vec<SessionBooking>>, Custom<String>> {
    if booking.person_id != claim.uid && !is_admin(&claim) {
        return Err(Custom(Status::Forbidden, "not allowed to cancel bookings for other users".to_string()));
    }
    let bookings_deleted = query_as("DELETE FROM booking WHERE person_id = $1 AND session_id = $2 RETURNING person_id, session_id")
        .bind(booking.person_id)
        .bind(booking.session_id)
        .fetch_all(&state.pool)
        .await
        .map_err(|e| Custom(Status::InternalServerError, e.to_string()))?;
    Ok(Json(bookings_deleted))
}

#[derive(Serialize, FromRow, Debug)]
pub struct SessionBookingFull {
    person_id: i64,
    person_name: String,
    person_email: String,
    session_id: i64
}

#[get("/bookings?<session_id>&<person_id>")]
pub async fn list_bookings(state: &State<AppState>, claim: Claims, session_id: Option<i64>, mut person_id: Option<i64>) -> Result<Json<Vec<SessionBookingFull>>, Custom<String>> {
    let mut qb = QueryBuilder::new("SELECT booking.person_id, person.name as person_name, person.email as person_email, booking.session_id \
            FROM booking, person \
            WHERE booking.person_id = person.id");

    // For non-admins, person_id is fixed to the current user, ignoring the query param
    if !is_admin(&claim) {
        person_id = Some(claim.uid);
    }

    if let Some(person_id) = person_id {
        qb.push(" AND booking.person_id = ");
        qb.push_bind(person_id);
    }
    if let Some(session_id) = session_id {
        qb.push(" AND booking.session_id = ");
        qb.push_bind(session_id);
    }

    let bookings = qb.build_query_as()
        .fetch_all(&state.pool)
        .await
        .map_err(|e| Custom(Status::InternalServerError, e.to_string()))?;
    Ok(Json(bookings))
}

fn is_admin(claims: &Claims) -> bool {
    claims.roles.contains(&"admin".to_string()) || claims.roles.contains(&"trainer".to_string())
}