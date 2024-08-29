use chrono::{DateTime, Utc};
use rocket::http::Status;
use rocket::response::status::Custom;
use rocket::serde::json::Json;
use rocket::State;
use serde::Serialize;
use sqlx::{FromRow, query_as};
use crate::AppState;
use crate::claims::Claims;

#[derive(FromRow, Serialize)]
pub struct PersonRow {
    id: i64,
    name: String,
    email: String,
    phone: Option<String>,
    pwd: Option<String>,
    roles: Option<String>
}

#[derive(FromRow, Serialize)]
pub struct SessionTypeRow {
    id: i32,
    name: String,
    requires_trainer: bool
}

#[derive(FromRow, Serialize)]
pub struct LocationRow {
    id: i32,
    name: String,
    address: String
}

#[derive(FromRow, Serialize)]
pub struct SessionRow {
    id: i64,
    datetime: DateTime<Utc>,
    duration_mins: i32,
    session_type_name: String,
    location_name: Option<String>,
    trainer_email: Option<String>,
    max_booking_count: Option<i64>,
    notes: Option<String>,
}

#[derive(FromRow, Serialize)]
pub struct BookingRow {
    person_email: String,
    session_datetime: DateTime<Utc>,
    session_location_name: Option<String>,
    session_trainer_email: Option<String>
}

#[derive(Serialize)]
pub struct AllTables {
    session_type: Vec<SessionTypeRow>,
    location: Vec<LocationRow>,
    person: Vec<PersonRow>,
    session: Vec<SessionRow>,
    booking: Vec<BookingRow>
}

#[get("/backup")]
pub async fn backup_all(state: &State<AppState>, claim: Claims) -> Result<Json<AllTables>, Custom<String>> {
    claim.assert_roles_contains("admin")?;
    Ok(Json(AllTables{
        session_type: session_type_table(state).await?,
        location: location_table(state).await?,
        person: person_table(state).await?,
        session: session_table(state).await?,
        booking: booking_table(state).await?
    }))
}

async fn person_table(state: &State<AppState>) -> Result<Vec<PersonRow>, Custom<String>> {
    query_as("SELECT * FROM person")
        .fetch_all(&state.pool)
        .await
        .map_err(|e| Custom(Status::InternalServerError, format!("person: {}", e)))
}

async fn session_type_table(state: &State<AppState>) -> Result<Vec<SessionTypeRow>, Custom<String>> {
    query_as("SELECT * FROM session_type")
        .fetch_all(&state.pool)
        .await
        .map_err(|e| Custom(Status::InternalServerError, format!("session_type: {}", e)))
}

async fn location_table(state: &State<AppState>) -> Result<Vec<LocationRow>, Custom<String>> {
    query_as("SELECT * FROM location")
        .fetch_all(&state.pool)
        .await
        .map_err(|e| Custom(Status::InternalServerError, format!("location: {}", e)))
}

async fn session_table(state: &State<AppState>) -> Result<Vec<SessionRow>, Custom<String>> {
    query_as("SELECT s.id, s.datetime, s.duration_mins, s.max_booking_count as max_booking_count, s.notes as notes, st.name as session_type_name, l.name as location_name, t.email as trainer_email \
            FROM session as s, session_type as st, location as l, person as t \
            WHERE s.session_type = st.id \
            AND s.location = l.id \
            AND s.trainer = t.id")
        .fetch_all(&state.pool)
        .await
        .map_err(|e| Custom(Status::InternalServerError, format!("session: {}", e)))
}

async fn booking_table(state: &State<AppState>) -> Result<Vec<BookingRow>, Custom<String>> {
    query_as("SELECT p.email AS person_email, s.datetime AS session_datetime, l.name AS session_location_name, t.email AS session_trainer_email \
            FROM booking as b \
            LEFT JOIN person AS p ON b.person_id = p.id \
            LEFT JOIN session AS s ON b.session_id = s.id \
            LEFT JOIN location AS l ON s.location = l.id \
            LEFT JOIN person AS t ON s.trainer = t.id")
        .fetch_all(&state.pool)
        .await
        .map_err(|e| Custom(Status::InternalServerError, format!("booking: {}", e)))
}