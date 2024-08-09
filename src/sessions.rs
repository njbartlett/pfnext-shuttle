use chrono::{DateTime, FixedOffset, Utc};
use rocket::form::validate::Contains;
use rocket::futures::TryFutureExt;
use rocket::http::hyper::body::HttpBody;
use rocket::http::Status;
use rocket::response::status::{Accepted, Created, Custom, NoContent, NotFound};
use rocket::serde::Deserialize;
use rocket::serde::json::Json;
use rocket::State;
use serde::Serialize;
use shuttle_runtime::__internals::tracing_subscriber::fmt::writer::OptionalWriter;
use sqlx::{Error, FromRow, PgPool, Postgres, query_as, QueryBuilder, Row};
use sqlx::postgres::{PgArguments, PgRow};
use sqlx::query::QueryAs;

use crate::{AppState, BigintRecord, log_info, parse_opt_date, SessionLocation, SessionTrainer, SessionType};
use crate::claims::Claims;

#[derive(Serialize, Clone, Debug)]
pub struct SessionFullRecord {
    id: i64,
    datetime: DateTime<Utc>,
    duration_mins: i32,
    session_type: SessionType,
    location: Option<SessionLocation>,
    trainer: Option<SessionTrainer>,
    booked: bool,
    booking_count: i64,
    max_booking_count: Option<i64>,
    notes: Option<String>
}

impl FromRow<'_, PgRow> for SessionFullRecord {
    fn from_row(row: &PgRow) -> Result<Self, Error> {
        let session_id: i64 = row.try_get("id")?;
        let trainer_id: Option<i64> = row.try_get("trainer_id").ok();
        let trainer: Option<SessionTrainer> = match trainer_id {
            Some(id) => Some(SessionTrainer {
                id,
                name: row.try_get("trainer_name")?,
                email: row.try_get("trainer_email")?,
            }),
            None => None
        };

        let location_id: Option<i32> = row.try_get("location_id").ok();
        let location: Option<SessionLocation> = match location_id {
            Some(id) => Some(SessionLocation{
                id,
                name: row.try_get("location_name")?,
                address: row.try_get("location_address")?
            }),
            None => None
        };

        Ok(SessionFullRecord {
            id: session_id,
            datetime: row.try_get("datetime")?,
            duration_mins: row.try_get("duration_mins")?,
            session_type: SessionType{
                id: row.try_get("session_type_id")?,
                name: row.try_get("session_type_name")?,
                requires_trainer: row.try_get("session_type_requires_trainer").ok().unwrap_or(true)
            },
            location,
            trainer,
            booked: row.try_get("booked").ok().unwrap_or(false),
            booking_count: row.try_get("booking_count")?,
            max_booking_count: row.try_get("max_booking_count").ok(),
            notes: row.try_get("notes").ok()
        })
    }
}

#[derive(Deserialize, Debug)]
struct NewSession {
    datetime: DateTime<Utc>,
    duration_mins: i32,
    session_type_id: i32,
    location_id: Option<i32>,
    trainer_id: Option<i64>,
    max_bookings: Option<i64>,
    notes: Option<String>
}

impl NewSession {
    async fn validate(self: &Self, pool: &PgPool) -> Result<(), String> {
        if self.trainer_id.is_none() {
            let session_type: SessionType = SessionType::find_by_id(pool, self.session_type_id)
                .await?
                .ok_or(format!("Session type not found with id {}", self.session_type_id))?;
            if session_type.requires_trainer {
                return Err(format!("Sessions of type '{}' require a trainer.", session_type.name));
            }
        }
        Ok(())
    }
}

#[get("/sessions?<from>&<to>&<trainer_id>")]
pub async fn list_sessions(state: &State<AppState>, claim: Claims, from: Option<String>, to: Option<String>, trainer_id: Option<i64>) -> Result<Json<Vec<SessionFullRecord>>, Custom<String>> {
    let mut qb: QueryBuilder<Postgres> = QueryBuilder::default();
    build_session_query(Some(claim.uid), from, to, trainer_id, &mut qb)?;
    qb.push(" ORDER BY s.datetime ASC");
    info!("build_session_query compiled SQL: {}", qb.sql());

    let sessions = qb.build_query_as()
        .fetch_all(&state.pool)
        .await
        .map_err(|e| Custom(Status::InternalServerError, e.to_string()))?;
    Ok(Json(sessions))
}

#[get("/sessions/<session_id>")]
pub async fn get_session(state: &State<AppState>, claim: Claims, session_id: i64) -> Result<Json<SessionFullRecord>, Custom<String>> {
    let mut qb: QueryBuilder<Postgres> = QueryBuilder::default();
    build_session_query(Some(claim.uid), None, None, None, &mut qb)?;
    qb.push(" WHERE s.id = ");
    qb.push_bind(session_id);
    info!("build_session_query compiled SQL: {}", qb.sql());

    qb.build_query_as()
        .fetch_optional(&state.pool)
        .await
        .map_err(|e| Custom(Status::InternalServerError, e.to_string()))?
        .ok_or_else(|| Custom(Status::NotFound, format!("session with id {} not found", session_id)))
        .map(|r| Json(r))
}

fn build_session_query<'a>(booking_person_id: Option<i64>, from: Option<String>, to: Option<String>, trainer_id: Option<i64>, qb: &'a mut QueryBuilder<Postgres>) -> Result<(), Custom<String>> {
    qb.push("SELECT s.id, s.datetime, s.duration_mins, s.notes, \
        t.id AS session_type_id, t.name AS session_type_name, t.requires_trainer AS session_type_requires_trainer, \
        loc.id AS location_id, loc.name AS location_name, loc.address AS location_address, \
        trainer.id AS trainer_id, trainer.name AS trainer_name, trainer.email AS trainer_email, \
        (SELECT COUNT(*) FROM booking WHERE booking.session_id = s.id) AS booking_count, s.max_booking_count as max_booking_count");

    if let Some(booking_person_id) = booking_person_id {
        qb.push(", CASE WHEN EXISTS (SELECT 1 FROM booking WHERE booking.session_id = s.id AND booking.person_id = ");
        qb.push_bind(booking_person_id);
        qb.push(") THEN true ELSE false END AS booked");
    }

    qb.push(" FROM session as s \
        INNER JOIN session_type AS t ON s.session_type = t.id \
        LEFT JOIN location AS loc ON s.location = loc.id \
        LEFT JOIN person AS trainer ON s.trainer = trainer.id");

    let parsed_from = parse_opt_date(from)?;
    let parsed_to = parse_opt_date(to)?;
    let mut operator: String = " WHERE".to_string();
    if let Some(from) = parsed_from {
        qb.push(operator + " s.datetime >= ");
        qb.push_bind(from);
        operator = " AND".to_string();
    }
    if let Some(to) = parsed_to {
        qb.push(operator + " s.datetime <= ");
        qb.push_bind(to);
        operator = " AND".to_string();
    }
    if let Some(trainer_id) = trainer_id {
        qb.push(operator + " trainer.id = ");
        qb.push_bind(trainer_id);
        operator = " AND".to_string();
    }
    Ok(())
}

#[post("/sessions", data="<new_session>")]
pub async fn create_session(
    state:  &State<AppState>,
    claims: Claims,
    new_session: Json<NewSession>
) -> Result<Created<Json<BigintRecord>>, Custom<String>> {
    // Admins can create any session. Trainers can only create sessions with themselves as the trainer.
    // Nobody else can create sessions.
    if !claims.has_role("admin") {
        if claims.has_role("trainer") {
            if !Some(claims.uid).eq(&new_session.trainer_id) {
                return Err(Custom(Status::Forbidden, "trainers can only create sessions for themselves".to_string()));
            }
        } else {
            return Err(Custom(Status::Forbidden, "only admins or trainers can create sessions".to_string()));
        }
    }

    new_session.validate(&state.pool)
        .await
        .map_err(|e| Custom(Status::BadRequest, e.to_string()))?;

    let id_record: BigintRecord = query_as("INSERT INTO session (datetime, duration_mins, session_type, location, trainer, max_booking_count, notes) VALUES ($1, $2, $3, $4, $5, $6, $7) RETURNING id")
        .bind(&new_session.datetime)
        .bind(&new_session.duration_mins)
        .bind(&new_session.session_type_id)
        .bind(&new_session.location_id)
        .bind(&new_session.trainer_id)
        .bind(&new_session.max_bookings)
        .bind(&new_session.notes)
        .fetch_optional(&state.pool)
        .await
        .map_err(|e| Custom(Status::InternalServerError, e.to_string()))?
        .ok_or_else(|| Custom(Status::Conflict, "no new record created".to_string()))?;
    info!("Created session id {}", id_record.id);
    Ok(Created::new(format!("/sessions/{}", id_record.id)).body(Json(id_record)))
}

#[delete("/sessions/<session_id>")]
pub async fn delete_session(state: &State<AppState>, claims: Claims, session_id: i64) -> Result<NoContent, Custom<String>> {
    let mut qb = QueryBuilder::new("DELETE FROM session WHERE id = ");
    qb.push_bind(session_id);

    if !claims.roles.contains(&"admin".to_string()) {
        if claims.roles.contains(&"trainer".to_string()) {
            qb.push(" AND trainer = ");
            qb.push_bind(claims.uid);
        } else {
            return Err(Custom(Status::Forbidden, "only admins and trainers can delete sessions".to_string()));
        }
    }
    qb.push(" RETURNING id");
    let id_record: BigintRecord= qb.build_query_as()
        .fetch_optional(&state.pool)
        .await
        .map_err(|e| Custom(Status::InternalServerError, e.to_string()))?
        .ok_or_else(|| Custom(Status::NotFound, format!("session id {} not found, or not deletable by current user", session_id)))?;
    info!("Deleted session id {}", id_record.id);

    Ok(NoContent)
}

#[put("/sessions/<session_id>", data="<new_session>")]
pub async fn update_session(
    state: &State<AppState>,
    claims: Claims,
    session_id: i64,
    new_session: Json<NewSession>
) -> Result<NoContent, Custom<String>> {
    let mut qb: QueryBuilder<Postgres> = QueryBuilder::new("UPDATE session SET datetime = ");
    qb.push_bind(new_session.datetime);

    qb.push(", duration_mins = ");
    qb.push_bind(new_session.duration_mins);

    qb.push(", session_type = ");
    qb.push_bind(new_session.session_type_id);

    qb.push(", location = ");
    qb.push_bind(new_session.location_id);

    qb.push(", trainer = ");
    qb.push_bind(new_session.trainer_id);

    qb.push(", max_booking_count = ");
    qb.push_bind(new_session.max_bookings);

    qb.push(", notes = ");
    qb.push_bind(&new_session.notes);

    qb.push(" WHERE id = ");
    qb.push_bind(session_id);

    if !claims.has_role("admin") {
        if claims.has_role("trainer") {
            qb.push(" AND trainer = ");
            qb.push_bind(claims.uid);
        } else {
            return Err(Custom(Status::NotFound, "only admins and trainers can update sessions".to_string()));
        }
    }
    qb.push(" RETURNING id");

    new_session.validate(&state.pool)
        .await
        .map_err(|e| Custom(Status::BadRequest, e.to_string()))?;

    let id_record: BigintRecord = qb.build_query_as()
        .fetch_optional(&state.pool)
        .await
        .map_err(|e| Custom(Status::InternalServerError, e.to_string()))?
        .ok_or_else(|| Custom(Status::NotFound, format!("session id {} not found, or not updatable by current user", session_id)))?;
    info!("Updating session id {} with data {:?}", id_record.id, new_session);
    Ok(NoContent)
}

#[get("/locations")]
pub async fn list_locations(state: &State<AppState>) -> Result<Json<Vec<SessionLocation>>, Custom<String>> {
    query_as("SELECT id, name, address FROM location")
        .fetch_all(&state.pool)
        .await
        .map_err(|e| Custom(Status::InternalServerError, e.to_string()))
        .map(|v| Json(v))
}

#[get("/session_types")]
pub async fn list_session_types(state: &State<AppState>) -> Result<Json<Vec<SessionType>>, Custom<String>> {
    query_as("SELECT id, name, requires_trainer FROM session_type ORDER BY requires_trainer DESC, name")
        .fetch_all(&state.pool)
        .await
        .map_err(|e| Custom(Status::InternalServerError, e.to_string()))
        .map(|v| Json(v))
}