use std::cmp::Ordering;
use crate::AppState;
use chrono::{DateTime, Utc};

use itertools::Itertools;

use rocket::State;
use rocket::response::status::BadRequest;
use rocket::serde::json::Json;

use serde::Serialize;
use sqlx::FromRow;

#[derive(Serialize, FromRow, Clone, Debug)]
struct Session {
    id: i64,
    datetime: DateTime<Utc>,
    duration_mins: i32,
    session_type: String,
    location: String
}

async fn fetch_sessions(state: &State<AppState>) -> Result<Vec<Session>, BadRequest<String>> {
    let sessions = sqlx::query_as("SELECT s.id, s.datetime, s.duration_mins, t.name as session_type, l.name as location \
            FROM session as s, session_type as t, location as l \
            WHERE s.session_type = t.id AND s.location = l.id")
        .fetch_all(&state.pool)
        .await
        .map_err(|e| BadRequest(e.to_string()))?;
    Ok(sessions)
}

#[get("/sessions")]
pub async fn list_sessions(state: &State<AppState>) -> Result<Json<Vec<Session>>, BadRequest<String>> {
    let sessions = fetch_sessions(state).await?;
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

impl Eq for SessionDate {

}

impl Ord for SessionDate {
    fn cmp(&self, other: &Self) -> Ordering {
        return self.date.cmp(&other.date);
    }
}

#[get("/sessions_by_date")]
pub async fn list_sessions_by_date(state: &State<AppState>) -> Result<Json<Vec<SessionDate>>, BadRequest<String>> {
    let session_dates: Vec<SessionDate> = fetch_sessions(state).await?
        .into_iter()
        .into_group_map_by(|s| s.datetime.naive_local().date())
        .into_iter()
        .map(|(k, v)| SessionDate { date: k, sessions: v })
        .sorted()
        .collect();
    Ok(Json(session_dates))
}