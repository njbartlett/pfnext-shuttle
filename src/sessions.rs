use chrono::{DateTime, Utc};
use rocket::form::validate::Contains;
use rocket::http::Status;
use rocket::response::status::{Created, Custom, NoContent};
use rocket::serde::Deserialize;
use rocket::serde::json::Json;
use rocket::State;
use serde::Serialize;
use sqlx::{Error, FromRow, PgPool, Postgres, query_as, QueryBuilder, Row};
use sqlx::postgres::PgRow;

use crate::{BigintRecord, parse_opt_date, SessionLocation, SessionTrainer, SessionType};
use crate::claims::Claims;

#[derive(Serialize, Clone, Debug)]
pub struct SessionFullRecord {
    pub id: i64,
    pub datetime: DateTime<Utc>,
    pub duration_mins: i32,
    pub session_type: SessionType,
    pub location: Option<SessionLocation>,
    pub trainer: Option<SessionTrainer>,
    pub booked: bool,
    pub attended: bool,
    pub booking_count: i64,
    pub max_booking_count: Option<i64>,
    pub attended_count: Option<i64>,
    pub notes: Option<String>,
    pub cost: i16
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
                url: row.try_get("trainer_url")?,
            }),
            None => None
        };

        let location_id: Option<i32> = row.try_get("location_id").ok();
        let location: Option<SessionLocation> = match location_id {
            Some(id) => Some(SessionLocation{
                id,
                name: row.try_get("location_name")?,
                address: row.try_get("location_address")?,
                url: row.try_get("location_url")?
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
                requires_trainer: row.try_get("session_type_requires_trainer").ok().unwrap_or(true),
                cost: row.try_get("session_type_cost")?,
                deprecated: row.try_get("session_type_deprecated")?
            },
            location,
            trainer,
            booked: row.try_get("booked").ok().unwrap_or(false),
            attended: row.try_get("attended").ok().unwrap_or(false),
            booking_count: row.try_get("booking_count")?,
            max_booking_count: row.try_get("max_booking_count").ok(),
            attended_count: row.try_get("attended_count").ok(),
            notes: row.try_get("notes").ok(),
            cost: row.try_get("cost")?
        })
    }
}

#[derive(Deserialize, Debug)]
pub struct NewSession {
    datetime: DateTime<Utc>,
    duration_mins: i32,
    session_type_id: i32,
    location_id: Option<i32>,
    trainer_id: Option<i64>,
    max_bookings: Option<i64>,
    notes: Option<String>,
    cost: i16
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

#[get("/sessions?<from>&<to>&<trainer_id>&<attended>")]
pub async fn list_sessions(
    pool: &State<PgPool>,
    claim: Option<Claims>,
    from: Option<String>, to: Option<String>, trainer_id: Option<i64>, attended: bool
) -> Result<Json<Vec<SessionFullRecord>>, Custom<String>> {
    let mut qb: QueryBuilder<Postgres> = QueryBuilder::default();
    let is_admin = claim.as_ref().map_or(false, |c| c.has_role("admin"));
    let uid: Option<i64> = claim.as_ref().map(|c| c.uid);

    if attended && !is_admin {
        return Err(Custom(Status::Forbidden, "attendance data only available to admins".to_string()));
    }
    build_session_query(uid, from, to, trainer_id, attended, &mut qb)?;
    qb.push(" ORDER BY s.datetime ASC");

    let sessions = qb.build_query_as()
        .fetch_all(pool.inner())
        .await
        .map_err(|e| Custom(Status::InternalServerError, e.to_string()))?;
    Ok(Json(sessions))
}

#[get("/sessions/<session_id>?<attended>")]
pub async fn get_session(
    pool: &State<PgPool>, claims: Claims,
    session_id: i64, attended: bool
) -> Result<Json<SessionFullRecord>, Custom<String>> {
    if attended && !claims.has_role("admin") {
        return Err(Custom(Status::Forbidden, "attendance data only available to admins".to_string()));
    }
    let mut qb: QueryBuilder<Postgres> = QueryBuilder::default();
    build_session_query(Some(claims.uid), None, None, None, attended, &mut qb)?;
    qb.push(" WHERE s.id = ");
    qb.push_bind(session_id);

    qb.build_query_as()
        .fetch_optional(pool.inner())
        .await
        .map_err(|e| Custom(Status::InternalServerError, e.to_string()))?
        .ok_or_else(|| Custom(Status::NotFound, format!("session with id {} not found", session_id)))
        .map(|r| Json(r))
}

fn build_session_query(
    booking_person_id: Option<i64>,
    from: Option<String>,
    to: Option<String>,
    trainer_id: Option<i64>,
    show_attended: bool,
    qb: &mut QueryBuilder<Postgres>
) -> Result<(), Custom<String>> {
    qb.push("SELECT s.id, s.datetime, s.duration_mins, s.notes, s.cost, \
        t.id AS session_type_id, t.name AS session_type_name, t.requires_trainer AS session_type_requires_trainer, t.cost AS session_type_cost, t.deprecated AS session_type_deprecated, \
        loc.id AS location_id, loc.name AS location_name, loc.address AS location_address, loc.url AS location_url, \
        trainer.id AS trainer_id, trainer.name AS trainer_name, trainer.email AS trainer_email, trainer.url AS trainer_url, \
        (SELECT COUNT(*) FROM booking WHERE booking.session_id = s.id) AS booking_count, s.max_booking_count AS max_booking_count");

    if show_attended {
        qb.push(", (SELECT COUNT(*) from booking WHERE booking.session_id = s.id AND booking.attended = true) AS attended_count");
    }

    if let Some(booking_person_id) = booking_person_id {
        qb.push(", CASE WHEN EXISTS (SELECT 1 FROM booking WHERE booking.session_id = s.id AND booking.person_id = ");
        qb.push_bind(booking_person_id);
        qb.push(") THEN true ELSE false END AS booked");

        qb.push(", CASE WHEN EXISTS (SELECT 1 FROM booking WHERE booking.session_id = s.id AND booking.attended = true AND booking.person_id = ");
        qb.push_bind(booking_person_id);
        qb.push(") THEN true ELSE false END AS attended");
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
    }
    Ok(())
}

#[post("/sessions", data="<new_session>")]
pub async fn create_session(
    pool:  &State<PgPool>,
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

    new_session.validate(pool)
        .await
        .map_err(|e| Custom(Status::BadRequest, e.to_string()))?;

    let id_record: BigintRecord = query_as("INSERT INTO session (datetime, duration_mins, session_type, location, trainer, max_booking_count, notes, cost) VALUES ($1, $2, $3, $4, $5, $6, $7, $8) RETURNING id")
        .bind(&new_session.datetime)
        .bind(&new_session.duration_mins)
        .bind(&new_session.session_type_id)
        .bind(&new_session.location_id)
        .bind(&new_session.trainer_id)
        .bind(&new_session.max_bookings)
        .bind(&new_session.notes)
        .bind(&new_session.cost)
        .fetch_optional(pool.inner())
        .await
        .map_err(|e| Custom(Status::InternalServerError, e.to_string()))?
        .ok_or_else(|| Custom(Status::Conflict, "no new record created".to_string()))?;
    info!("Created session id {}", id_record.id);
    Ok(Created::new(format!("/sessions/{}", id_record.id)).body(Json(id_record)))
}

#[delete("/sessions/<session_id>")]
pub async fn delete_session(pool: &State<PgPool>, claims: Claims, session_id: i64) -> Result<NoContent, Custom<String>> {
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
        .fetch_optional(pool.inner())
        .await
        .map_err(|e| Custom(Status::InternalServerError, e.to_string()))?
        .ok_or_else(|| Custom(Status::NotFound, format!("session id {} not found, or not deletable by current user", session_id)))?;
    info!("Deleted session id {}", id_record.id);

    Ok(NoContent)
}

#[put("/sessions/<session_id>", data="<new_session>")]
pub async fn update_session(
    pool: &State<PgPool>,
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

    qb.push(", cost = ");
    qb.push_bind(new_session.cost);

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

    new_session.validate(pool)
        .await
        .map_err(|e| Custom(Status::BadRequest, e.to_string()))?;

    let id_record: BigintRecord = qb.build_query_as()
        .fetch_optional(pool.inner())
        .await
        .map_err(|e| Custom(Status::InternalServerError, e.to_string()))?
        .ok_or_else(|| Custom(Status::NotFound, format!("session id {} not found, or not updatable by current user", session_id)))?;
    info!("Updating session id {} with data {:?}", id_record.id, new_session);
    Ok(NoContent)
}

#[get("/locations")]
pub async fn list_locations(pool: &State<PgPool>) -> Result<Json<Vec<SessionLocation>>, Custom<String>> {
    query_as("SELECT id, name, address, url FROM location")
        .fetch_all(pool.inner())
        .await
        .map_err(|e| Custom(Status::InternalServerError, e.to_string()))
        .map(|v| Json(v))
}

#[get("/session_types?<deprecated>")]
pub async fn list_session_types(pool: &State<PgPool>, deprecated: Option<bool>) -> Result<Json<Vec<SessionType>>, Custom<String>> {
    let mut qb: QueryBuilder<Postgres> = QueryBuilder::new("SELECT id, name, requires_trainer, cost, deprecated FROM session_type");
    if let Some(deprecated) = deprecated {
        qb.push(" WHERE deprecated = ");
        qb.push_bind(deprecated);
    }
    qb.push(" ORDER BY requires_trainer DESC, name");

    qb.build_query_as()
        .fetch_all(pool.inner())
        .await
        .map_err(|e| Custom(Status::InternalServerError, e.to_string()))
        .map(Json::from)
}

#[cfg(test)]
mod tests {
    use rocket::State;
    use sqlx::{query_as, Executor, FromRow, PgPool};
    use crate::sessions::list_sessions;

    #[derive(FromRow)]
    struct BigintRecord {
        id: i64
    }

    #[sqlx::test]
    async fn browse_sessions_not_logged_in(pool: PgPool) {
        pool.execute(include_str!("../schema.sql")).await.unwrap();
        let _session_id: BigintRecord = query_as("insert into session (datetime, duration_mins, session_type) values ('2025-01-01 00:00:00+0', 60, 1) returning id")
            .fetch_one(&pool)
            .await.unwrap();
        let sessions = list_sessions(State::from(&pool), None, None, None, None, false).await.unwrap();
        assert_eq!(1, sessions.len());
    }

}