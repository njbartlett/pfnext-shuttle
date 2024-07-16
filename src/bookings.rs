use chrono::{DateTime, Utc};
use rocket::futures::{Stream, StreamExt};
use rocket::futures::stream::BoxStream;
use rocket::http::Status;
use rocket::response::status::{Created, Custom, NoContent};
use rocket::serde::json::Json;
use rocket::serde::Serialize;
use rocket::State;
use serde::Deserialize;
use sqlx::{Acquire, Error, Executor, FromRow, Postgres, query_as, QueryBuilder, raw_sql, Row};
use sqlx::postgres::{PgArguments, PgQueryResult, PgRow};
use sqlx::query::{Query, QueryAs};

use crate::{AppState, CountResult, parse_opt_date, SessionLocation, SessionType};
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
    session_type: SessionType,
    attended: bool
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
            },
            attended: row.try_get("attended").ok().unwrap_or(false)
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
                s.datetime as session_datetime, s.duration_mins as session_duration_mins, s.location as session_location_id, l.name as session_location_name, l.address as session_location_address, \
                s.session_type as session_type_id, t.name as session_type_name, b.attended as attended \
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

    qb.push(" ORDER BY session_datetime, person_name");
    let bookings = qb.build_query_as()
        .fetch_all(&state.pool)
        .await
        .map_err(|e| Custom(Status::InternalServerError, e.to_string()))?;
    Ok(Json(bookings))
}

async fn take_result_from_stream<'a>(stream: &mut BoxStream<'a, Result<PgQueryResult, Error>>) -> Result<PgQueryResult, Custom<String>> {
    stream.next()
        .await
        .ok_or(Custom(Status::InternalServerError, "no more results".to_string()))?
        .map_err(|e| Custom(Status::InternalServerError, e.to_string()))
}

#[post("/bookings", data="<booking>")]
pub async fn create_booking(state: &State<AppState>, claim: Claims, booking: Json<SessionBooking>) -> Result<Created<Json<SessionBooking>>, Custom<String>> {
    claim.assert_roles_contains("member")?;
    if booking.person_id != claim.uid && !claim.has_role("admin") {
        return Err(Custom(Status::Forbidden, "not allowed to create bookings for other users".to_string()));
    }

    // Read the max_booking_count for the session if present
    let session_with_max_booking_count: SessionWithMaxBookingCount = query_as("SELECT id, max_booking_count FROM session WHERE id = $1")
        .bind(&booking.session_id)
        .fetch_optional(&state.pool)
        .await
        .map_err(|e| Custom(Status::InternalServerError, e.to_string()))?
        .ok_or(Custom(Status::NotFound, format!("no session with id {}", &booking.session_id)))?;
    let result = match session_with_max_booking_count.max_booking_count {
        Some(max_booking_count) => book_session_with_max_bookings(state, booking.person_id, booking.session_id, max_booking_count).await,
        None => book_session_no_max_bookings(state, booking.person_id, booking.session_id).await
    };

    info!("Created booking: {:?}", &booking);
    Ok(Created::new(format!("/bookings?sessionid={},person_id={}", booking.session_id, booking.person_id)))
}

async fn book_session_no_max_bookings(state: &State<AppState>, person_id: i64, session_id: i64) -> Result<(), Custom<String>> {
    query_as("INSERT INTO booking (person_id, session_id) VALUES ($1, $2) RETURNING person_id, session_id")
        .bind(person_id)
        .bind(session_id)
        .fetch_one(&state.pool)
        .await
        .map_err(|e| Custom(Status::InternalServerError, e.to_string()))
}

#[derive(FromRow)]
struct SessionWithMaxBookingCount {
    id: i64,
    max_booking_count: Option<i64>
}


async fn book_session_with_max_bookings(state: &State<AppState>, person_id: i64, session_id: i64, max_bookings: i64) -> Result<(), Custom<String>> {
    // Atomically update the booking table to insert a new booking if and only if the count of
    // bookings for the referenced session is less than the maximum. Adapted from this StackOverflow
    // answer: https://dba.stackexchange.com/a/167283
    // NB simple string interpolation without prepared statements is safe because the arguments all
    // are numeric.
    let sql = format!("BEGIN; \
        SELECT id FROM session WHERE id = {} FOR NO KEY UPDATE; \
        INSERT INTO booking (person_id, session_id) \
        SELECT {}, {} FROM booking \
        WHERE session_id = {} \
        HAVING count(*) < {} \
        ON CONFLICT DO NOTHING \
        RETURNING person_id, session_id; \
        END;", session_id, person_id, session_id, session_id, max_bookings);
    info!("Executing raw SQL: {}", &sql);
    let mut result_stream = raw_sql(sql.as_str()).execute_many(&state.pool);

    let _ = take_result_from_stream(&mut result_stream).await?; // result from BEGIN;
    let _ = take_result_from_stream(&mut result_stream).await?; // result from SELECT..FOR UPDATE;
    let insert_result = take_result_from_stream(&mut result_stream).await?; // result from INSERT..RETURNING;
    let _ = take_result_from_stream(&mut result_stream).await?; // result from COMMIT;
    info!("Insert result: {:?}", insert_result);

    if insert_result.rows_affected() == 0 {
        return Err(Custom(Status::Conflict, format!("session has reached it maximum number of bookings: {}", max_bookings)));
    }
    Ok(())
}

#[delete("/bookings?<session_id>&<person_id>")]
pub async fn delete_booking(state: &State<AppState>, claim: Claims, person_id: i64, session_id: i64) -> Result<Json<SessionBooking>, Custom<String>> {
    claim.assert_roles_contains("member")?;
    if person_id != claim.uid && !claim.has_role("admin") {
        return Err(Custom(Status::Forbidden, "not allowed to cancel bookings for other users".to_string()));
    }
    let booking_deleted = query_as("DELETE FROM booking WHERE person_id = $1 AND session_id = $2 RETURNING person_id, session_id")
        .bind(person_id)
        .bind(session_id)
        .fetch_optional(&state.pool)
        .await
        .map_err(|e| Custom(Status::InternalServerError, e.to_string()))?
        .ok_or(Custom(Status::NotFound, format!("no booking found with person_id={} and session_id={}", person_id, session_id)))?;
    Ok(Json(booking_deleted))
}

#[derive(Deserialize)]
pub struct BookingUpdate {
    attended: bool
}

#[put("/bookings?<session_id>&<person_id>", data="<booking_update>")]
pub async fn update_booking(state: &State<AppState>, claim: Claims, person_id: i64, session_id: i64, booking_update: Json<BookingUpdate>) -> Result<NoContent, Custom<String>> {
    claim.assert_roles_contains("admin")?;
    let _ = query_as("UPDATE booking SET attended = $1 WHERE person_id = $2 AND session_id = $3 RETURNING person_id, session_id")
        .bind(booking_update.attended)
        .bind(person_id)
        .bind(session_id)
        .fetch_optional(&state.pool)
        .await
        .map_err(|e| Custom(Status::InternalServerError, e.to_string()))?
        .ok_or(Custom(Status::NotFound, format!("no booking found with person_id={} and session_id={}", person_id, session_id)))?;
    Ok(NoContent)
}