use chrono::{DateTime, Utc};
use rocket::http::Status;
use rocket::response::status::Custom;
use rocket::serde::json::Json;
use rocket::serde::Serialize;
use rocket::State;
use serde::Deserialize;
use sqlx::{Error, FromRow, query_as, QueryBuilder, Row};
use sqlx::postgres::PgRow;

use crate::{AppState, parse_opt_date, SessionLocation, SessionType};
use crate::claims::Claims;

#[derive(Serialize, Deserialize, FromRow, Debug)]
pub struct SessionBooking {
    person_id: i64,
    session_id: i64
}

#[derive(Serialize, Debug)]
pub struct SessionBookingFull {
    person_id: i64,
    person_name: String,
    person_email: String,
    session_id: i64,
    session_datetime: DateTime<Utc>,
    session_duration_mins: i32,
    session_location: SessionLocation,
    session_type: SessionType
}

impl FromRow<'_, PgRow> for SessionBookingFull {
    fn from_row(row: &'_ PgRow) -> Result<Self, Error> {
        Ok(SessionBookingFull {
            person_id: row.try_get("person_id")?,
            person_name: row.try_get("person_name")?,
            person_email: row.try_get("person_email")?,
            session_id: row.try_get("session_id")?,
            session_datetime: row.try_get("session_datetime")?,
            session_duration_mins: row.try_get("session_duration_mins")?,
            session_location: SessionLocation {
                id: row.try_get("session_location_id")?,
                name: row.try_get("session_location_name")?,
                address: row.try_get("session_location_address")?,
            },
            session_type: SessionType{
                id: row.try_get("session_type_id")?,
                name: row.try_get("session_type_name")?
            }
        })
    }
}

#[get("/bookings?<session_id>&<person_id>&<from>&<to>")]
pub async fn list_bookings(
    state: &State<AppState>,
    claim: Claims,
    session_id: Option<i64>,
    person_id: Option<i64>,
    from: Option<String>,
    to: Option<String>
) -> Result<Json<Vec<SessionBookingFull>>, Custom<String>> {
    let mut qb = QueryBuilder::new("SELECT b.person_id, p.name as person_name, p.email as person_email, b.session_id, \
                s.datetime as session_datetime, s.duration_mins as session_duration_mins, s.location as session_location_id, l.name as session_location_name, l.address as session_location_address,\
                s.session_type as session_type_id, t.name as session_type_name \
            FROM booking as b, person as p, session as s, location as l, session_type as t \
            WHERE b.person_id = p.id \
            AND b.session_id = s.id \
            AND s.location = l.id \
            AND s.session_type = t.id");

    if let Some(person_id) = person_id {
        if person_id != claim.uid && !claim.has_role("admin") {
            return Err(Custom(Status::Forbidden, "only admins can view bookings for other users".to_string()))
        }
        qb.push(" AND b.person_id = ");
        qb.push_bind(person_id);
    } else if !claim.has_role("admin") {
        return Err(Custom(Status::Forbidden, "only admins can view bookings for other users".to_string()))
    }

    if let Some(session_id) = session_id {
        qb.push(" AND b.session_id = ");
        qb.push_bind(session_id);
    }
    if let Some(from) = parse_opt_date(from)? {
        qb.push(" AND s.datetime >= ");
        qb.push_bind(from);
    }
    if let Some(to) = parse_opt_date(to)? {
        qb.push(" AND s.datetime <= ");
        qb.push_bind(to);
    }

    qb.push(" ORDER BY session_datetime");
    let bookings = qb.build_query_as()
        .fetch_all(&state.pool)
        .await
        .map_err(|e| Custom(Status::InternalServerError, e.to_string()))?;
    Ok(Json(bookings))
}

#[post("/bookings", data="<booking>")]
pub async fn create_booking(state: &State<AppState>, claim: Claims, booking: Json<SessionBooking>) -> Result<Json<Option<SessionBooking>>, Custom<String>> {
    claim.assert_roles_contains("member")?;
    if booking.person_id != claim.uid && !claim.has_role("admin") {
        return Err(Custom(Status::Forbidden, "not allowed to create bookings for other users".to_string()));
    }
    let booking_created = query_as("INSERT INTO booking (person_id, session_id) VALUES ($1, $2) ON CONFLICT DO NOTHING RETURNING person_id, session_id")
        .bind(booking.person_id)
        .bind(booking.session_id)
        .fetch_optional(&state.pool)
        .await
        .map_err(|e| Custom(Status::InternalServerError, e.to_string()))?;
    info!("Created booking: {:?}", booking_created);
    Ok(Json(booking_created))
}

#[delete("/bookings?<session_id>&<person_id>")]
pub async fn delete_booking(state: &State<AppState>, claim: Claims, person_id: i64, session_id: i64) -> Result<Json<Option<SessionBooking>>, Custom<String>> {
    claim.assert_roles_contains("member")?;
    if person_id != claim.uid && !claim.has_role("admin") {
        return Err(Custom(Status::Forbidden, "not allowed to cancel bookings for other users".to_string()));
    }
    let booking_deleted = query_as("DELETE FROM booking WHERE person_id = $1 AND session_id = $2 RETURNING person_id, session_id")
        .bind(person_id)
        .bind(session_id)
        .fetch_optional(&state.pool)
        .await
        .map_err(|e| Custom(Status::InternalServerError, e.to_string()))?;
    info!("Deleted booking(s): {:?}", booking_deleted);
    Ok(Json(booking_deleted))
}