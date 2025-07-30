use chrono::{DateTime, Utc};
use rocket::http::Status;
use rocket::response::status::{Created, Custom, NoContent};
use rocket::serde::json::Json;
use rocket::serde::Deserialize;
use rocket::{Route, State};
use serde::Serialize;
use sqlx::postgres::{PgArguments, PgRow};
use sqlx::{query, query_as, Arguments, Error, Execute, FromRow, PgPool, Postgres, QueryBuilder, Row};

use crate::common::parse_opt_date;
use crate::loginsession::LoginSession;

pub fn routes() -> Vec<Route> {
    routes![
        list_sessions,
        get_session,
        create_session,
        delete_session,
        list_locations,
        list_session_types,
        update_session,
    ]
}

#[derive(FromRow, Serialize, Clone, Debug, PartialEq)]
pub struct SessionType {
    pub id: i32,
    pub name: String,
    pub requires_trainer: bool,
    pub cost: i16,
    pub deprecated: bool
}

impl SessionType {
    async fn find_by_id(pool: &PgPool, id: i32) -> Result<Option<Self>, String> {
        query_as("SELECT * FROM session_type WHERE id = $1")
            .bind(id)
            .fetch_optional(pool)
            .await
            .map_err(|e| e.to_string())
    }
}

#[derive(Serialize, Clone, Debug)]
struct SessionTrainer {
    id: i64,
    name: String,
    email: String,
    url: Option<String>
}

#[derive(FromRow, Serialize, Clone, Debug, PartialEq)]
pub struct SessionLocation {
    pub id: i32,
    pub name: String,
    pub address: String,
    pub url: Option<String>
}

#[derive(Serialize, Clone, Debug)]
struct SessionFullRecord {
    id: i64,
    datetime: DateTime<Utc>,
    duration_mins: i32,
    session_type: SessionType,
    location: Option<SessionLocation>,
    trainer: Option<SessionTrainer>,
    booked: bool,
    waitlist_rank: Option<i64>,
    attended: bool,
    rating: Option<i16>,
    booking_count: i64,
    avg_rating: Option<f64>,
    count_rating_all: i64,
    count_rating_5: i64,
    count_rating_4: i64,
    count_rating_3: i64,
    count_rating_2: i64,
    count_rating_1: i64,
    max_booking_count: Option<i64>,
    attended_count: Option<i64>,
    notes: Option<String>,
    cost: i16
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

        let count_rating_5 = row.try_get::<i64, &str>("count_rating_5")?;
        let count_rating_4 = row.try_get::<i64, &str>("count_rating_4")?;
        let count_rating_3 = row.try_get::<i64, &str>("count_rating_3")?;
        let count_rating_2 = row.try_get::<i64, &str>("count_rating_2")?;
        let count_rating_1 = row.try_get::<i64, &str>("count_rating_1")?;
        let count_rating_all = count_rating_1 + count_rating_2 + count_rating_3 + count_rating_4 + count_rating_5;
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
            waitlist_rank: row.try_get("waitlist_rank").ok(),
            attended: row.try_get("attended").ok().unwrap_or(false),
            rating: row.try_get("rating").ok(),
            booking_count: row.try_get("booking_count")?,
            avg_rating: row.try_get("avg_rating")?,
            count_rating_all, count_rating_5, count_rating_4, count_rating_3, count_rating_2, count_rating_1,
            max_booking_count: row.try_get("max_booking_count").ok(),
            attended_count: row.try_get("attended_count").ok(),
            notes: row.try_get("notes").ok(),
            cost: row.try_get("cost")?,
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
async fn list_sessions(
    pool: &State<PgPool>,
    login: Option<LoginSession>,
    from: Option<String>, to: Option<String>, trainer_id: Option<i64>, attended: bool
) -> Result<Json<Vec<SessionFullRecord>>, Custom<String>> {
    let mut qb: QueryBuilder<Postgres> = QueryBuilder::default();
    let is_admin = login.as_ref().map_or(false, |c| c.has_role("admin"));
    let uid: Option<i64> = login.as_ref().map(|c| c.uid);

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
async fn get_session(
    pool: &State<PgPool>, login: LoginSession,
    session_id: i64, attended: bool
) -> Result<Json<SessionFullRecord>, Custom<String>> {
    if attended && !login.has_role("admin") {
        return Err(Custom(Status::Forbidden, "attendance data only available to admins".to_string()));
    }
    let mut qb: QueryBuilder<Postgres> = QueryBuilder::default();
    build_session_query(Some(login.uid), None, None, None, attended, &mut qb)?;
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
    qb.push("SELECT s.id, s.datetime, s.duration_mins, s.notes, s.cost,
        t.id AS session_type_id, t.name AS session_type_name, t.requires_trainer AS session_type_requires_trainer, t.cost AS session_type_cost, t.deprecated AS session_type_deprecated,
        loc.id AS location_id, loc.name AS location_name, loc.address AS location_address, loc.url AS location_url,
        trainer.id AS trainer_id, trainer.name AS trainer_name, trainer.email AS trainer_email, trainer.url AS trainer_url,
        (SELECT COUNT(*) FROM booking WHERE booking.session_id = s.id) AS booking_count, s.max_booking_count AS max_booking_count,
        (SELECT CAST(AVG(rating) AS FLOAT8) FROM booking WHERE booking.session_id = s.id) AS avg_rating,
        (SELECT COUNT(*) FROM booking WHERE booking.session_id = s.id AND rating = 5) AS count_rating_5,
        (SELECT COUNT(*) FROM booking WHERE booking.session_id = s.id AND rating = 4) AS count_rating_4,
        (SELECT COUNT(*) FROM booking WHERE booking.session_id = s.id AND rating = 3) AS count_rating_3,
        (SELECT COUNT(*) FROM booking WHERE booking.session_id = s.id AND rating = 2) AS count_rating_2,
        (SELECT COUNT(*) FROM booking WHERE booking.session_id = s.id AND rating = 1) AS count_rating_1");

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

        qb.push(", (SELECT rating FROM booking WHERE booking.session_id = s.id AND booking.person_id = ");
        qb.push_bind(booking_person_id);
        qb.push(") AS rating");

        qb.push(", (SELECT waitlist_rank FROM (SELECT ROW_NUMBER() OVER (ORDER BY w.id ASC) AS waitlist_rank, person_id, session_id FROM waitlist w WHERE w.session_id = s.id ORDER BY w.id ASC) AS waitlist_sub WHERE waitlist_sub.person_id = ");
        qb.push_bind(booking_person_id);
        qb.push(")");
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
async fn create_session(
    pool:  &State<PgPool>,
    login: LoginSession,
    new_session: Json<NewSession>
) -> Result<Created<Json<i64>>, Custom<String>> {
    // Admins can create any session. Trainers can only create sessions with themselves as the trainer.
    // Nobody else can create sessions.
    if !login.has_role("admin") {
        if login.has_role("trainer") {
            if !Some(login.uid).eq(&new_session.trainer_id) {
                return Err(Custom(Status::Forbidden, "trainers can only create sessions for themselves".to_string()));
            }
        } else {
            return Err(Custom(Status::Forbidden, "only admins or trainers can create sessions".to_string()));
        }
    }

    new_session.validate(pool)
        .await
        .map_err(|e| Custom(Status::BadRequest, e.to_string()))?;

    let id_row = query("INSERT INTO session (datetime, duration_mins, session_type, location, trainer, max_booking_count, notes, cost) VALUES ($1, $2, $3, $4, $5, $6, $7, $8) RETURNING id")
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
    let id = id_row.try_get("id").map_err(|e| Custom(Status::InternalServerError, e.to_string()))?;
    info!("Created session id {}", id);
    Ok(Created::new(format!("/sessions/{}", id)).body(Json(id)))
}

#[delete("/sessions/<session_id>")]
async fn delete_session(pool: &State<PgPool>, login: LoginSession, session_id: i64) -> Result<NoContent, Custom<String>> {
    let mut qb = QueryBuilder::new("DELETE FROM session WHERE id = ");
    qb.push_bind(session_id);

    if !login.is_admin() {
        if login.has_role("trainer") {
            qb.push(" AND trainer = ");
            qb.push_bind(login.uid);
        } else {
            return Err(Custom(Status::Forbidden, "only admins and trainers can delete sessions".to_string()));
        }
    }
    qb.push(" RETURNING id");
    let id: i64 = qb.build()
        .fetch_optional(pool.inner())
        .await
        .map_err(|e| Custom(Status::InternalServerError, e.to_string()))?
        .ok_or_else(|| Custom(Status::NotFound, format!("session id {} not found, or not deletable by current user", session_id)))?
        .try_get("id")
        .map_err(|e| Custom(Status::InternalServerError, e.to_string()))?;
    info!("Deleted session id {}", id);

    Ok(NoContent)
}

#[put("/sessions/<session_id>", data="<new_session>")]
async fn update_session(
    pool: &State<PgPool>,
    login: LoginSession,
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

    if !login.has_role("admin") {
        if login.has_role("trainer") {
            qb.push(" AND trainer = ");
            qb.push_bind(login.uid);
        } else {
            return Err(Custom(Status::NotFound, "only admins and trainers can update sessions".to_string()));
        }
    }
    qb.push(" RETURNING id");

    new_session.validate(pool)
        .await
        .map_err(|e| Custom(Status::BadRequest, e.to_string()))?;

    let id: i64 = qb.build()
        .fetch_optional(pool.inner())
        .await
        .map_err(|e| Custom(Status::InternalServerError, e.to_string()))?
        .ok_or_else(|| Custom(Status::NotFound, format!("session id {} not found, or not updatable by current user", session_id)))?
        .try_get("id").map_err(|e| Custom(Status::InternalServerError, e.to_string()))?;
    info!("Updating session id {} with data {:?}", id, new_session);
    Ok(NoContent)
}

#[get("/locations")]
async fn list_locations(pool: &State<PgPool>) -> Result<Json<Vec<SessionLocation>>, Custom<String>> {
    query_as("SELECT id, name, address, url FROM location")
        .fetch_all(pool.inner())
        .await
        .map_err(|e| Custom(Status::InternalServerError, e.to_string()))
        .map(|v| Json(v))
}

#[get("/session_types?<deprecated>")]
async fn list_session_types(pool: &State<PgPool>, deprecated: Option<bool>) -> Result<Json<Vec<SessionType>>, Custom<String>> {
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
    use chrono::{Days, Utc};
    use rocket::State;
    use sqlx::{query, query_scalar, PgPool, Postgres};
    use crate::{loginsession::{LoginSession, Roles}, sessions::{list_sessions, SessionFullRecord}, users::UserLoginRecord};

    #[sqlx::test(fixtures("../schema.sql"))]
    async fn browse_sessions_not_logged_in(pool: PgPool) {
        query_scalar::<Postgres, i64>("insert into session (datetime, duration_mins, session_type) values ('2025-01-01 00:00:00+0', 60, 1) returning id")
            .fetch_one(&pool)
            .await.unwrap();
        let sessions = list_sessions(State::from(&pool), None, None, None, None, false).await.unwrap();
        assert_eq!(1, sessions.len());
    }

    #[sqlx::test(fixtures("../schema.sql", "fixtures/users.sql", "fixtures/sessions.sql"))]
    async fn session_list_includes_waitlist_rank(pool: PgPool) {
        let user1 = UserLoginRecord::load_by_email(&pool, "user1@example.com").await.unwrap().unwrap();

        // Session with no waitlist
        let login = create_login(user1.id, &user1.name, "member");
        let session= list_sessions(
            State::from(&pool),
            Some(login.clone()),
            Some("2025-01-01T00:00:00Z".to_string()),
            Some("2025-01-01T23:59:59Z".to_string()),
            None,
            false
        ).await.unwrap().0.into_iter().next().unwrap();
        assert_eq!(None, session.waitlist_rank, "waitlist rank should be null (not on waitlist)");

        // Add 1 other member and ourselves to the waitlist
        query("INSERT INTO waitlist (person_id, session_id) SELECT p.id, $1 FROM person AS p WHERE p.email = 'user2@example.com'")
            .bind(session.id)
            .execute(&pool).await.unwrap();
        query("INSERT INTO waitlist (person_id, session_id) VALUES ($1, $2)")
            .bind(user1.id)
            .bind(session.id)
            .execute(&pool).await.unwrap();

        // Requery sessions
        let session= list_sessions(
            State::from(&pool),
            Some(login),
            Some("2025-01-01T00:00:00Z".to_string()),
            Some("2025-01-01T23:59:59Z".to_string()),
            None,
            false
        ).await.unwrap().0.into_iter().next().unwrap();
        assert_eq!(Some(2), session.waitlist_rank, "waitlist rank should be 2nd");

    }

    #[sqlx::test(fixtures("../schema.sql", "fixtures/users.sql", "fixtures/sessions.sql"))]
    async fn session_ratings(pool: PgPool) {
        let user1 = UserLoginRecord::load_by_email(&pool, "user1@example.com").await.unwrap().unwrap();
        let user2 = UserLoginRecord::load_by_email(&pool, "user2@example.com").await.unwrap().unwrap();
        let session_id: i64 = query_scalar("SELECT id FROM session")
            .fetch_one(&pool)
            .await.unwrap();
        query("INSERT INTO booking (person_id, session_id, attended, rating) VALUES ($1, $2, true, 3) RETURNING session_id")
            .bind(user1.id).bind(session_id)
            .fetch_one(&pool)
            .await.unwrap();
        query("INSERT INTO booking (person_id, session_id, attended, rating) VALUES ($1, $2, true, 4) RETURNING session_id")
            .bind(user2.id).bind(session_id)
            .fetch_one(&pool)
            .await.unwrap();


        let login = create_login(user1.id, &user1.name, "member");
        let session= list_sessions(
            State::from(&pool),
            Some(login.clone()),
            Some("2025-01-01T00:00:00Z".to_string()),
            Some("2025-01-01T23:59:59Z".to_string()),
            None,
            false
        ).await.unwrap().0.into_iter().next().unwrap();
        assert_eq!(Some(3.5), session.avg_rating);
        assert_eq!(0, session.count_rating_1);
        assert_eq!(0, session.count_rating_2);
        assert_eq!(1, session.count_rating_3);
        assert_eq!(1, session.count_rating_4);
        assert_eq!(0, session.count_rating_5);
    }

    fn create_login(uid: i64, name: &str, role: &str) -> LoginSession {
        LoginSession {
            sessionid: "xxx".to_string(),
            uid,
            name: name.to_string(),
            email: format!("{}@example.org", name),
            roles: Roles::parse(role),
            loggedin: None, loggedin_from: None,
            expiry: Utc::now().checked_add_days(Days::new(1)).unwrap().fixed_offset()
        }
    }


}