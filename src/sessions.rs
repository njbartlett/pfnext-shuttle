use actix_web::{error, get, web};
use actix_web::web::Json;
use chrono::{DateTime, Utc};
use itertools::Itertools;
use serde::Serialize;
use sqlx::FromRow;
use crate::AppState;

#[derive(Serialize, FromRow, Clone, Debug)]
struct Session {
    id: i64,
    datetime: DateTime<Utc>,
    duration_mins: i32,
    session_type: String,
    location: String
}

async fn fetch_sessions(state: web::Data<AppState>) -> actix_web::Result<Vec<Session>> {
    let sessions = sqlx::query_as("SELECT s.id, s.datetime, s.duration_mins, t.name as session_type, l.name as location \
            FROM session as s, session_type as t, location as l \
            WHERE s.session_type = t.id AND s.location = l.id \
            ORDER BY s.datetime")
        .fetch_all(&state.pool)
        .await
        .map_err(|e| error::ErrorBadRequest(e.to_string()))?;
    Ok(sessions)
}

#[get("sessions")]
async fn list_sessions(state: web::Data<AppState>) -> actix_web::Result<Json<Vec<Session>>> {
    let sessions = fetch_sessions(state).await?;
    Ok(Json(sessions))
}

#[derive(Serialize, Debug)]
struct SessionDate {
    date: chrono::NaiveDate,
    sessions: Vec<Session>
}

#[get("sessions_by_date")]
async fn list_session_by_date(state: web::Data<AppState>) -> actix_web::Result<Json<Vec<SessionDate>>> {
    let session_dates: Vec<SessionDate> = fetch_sessions(state).await?
        .into_iter()
        .into_group_map_by(|s| s.datetime.naive_local().date())
        .into_iter()
        .map(|(k, v)| SessionDate { date: k, sessions: v })
        .collect();
    Ok(Json(session_dates))
}