use chrono::{DateTime, Utc};
use rocket::http::Status;
use rocket::response::status::Custom;
use rocket::serde::json::Json;
use rocket::State;
use serde::Serialize;
use sqlx::{FromRow, PgPool, query_as};
use crate::claims::Claims;

#[derive(FromRow, Serialize, Debug)]
pub struct PersonRow {
    id: i64,
    name: String,
    email: String,
    phone: Option<String>,
    pwd: Option<String>,
    roles: Option<String>,
    credits: i16
}

#[derive(FromRow, Serialize, Debug)]
pub struct SessionTypeRow {
    id: i32,
    name: String,
    requires_trainer: bool,
    cost: i16,
    deprecated: bool
}

#[derive(FromRow, Serialize, Debug)]
pub struct LocationRow {
    id: i32,
    name: String,
    address: String,
    url: Option<String>
}

#[derive(FromRow, Serialize, Debug)]
pub struct SessionRow {
    id: i64,
    datetime: DateTime<Utc>,
    duration_mins: i32,
    session_type_name: String,
    location_name: Option<String>,
    trainer_email: Option<String>,
    max_booking_count: Option<i64>,
    notes: Option<String>,
    cost: i16
}

#[derive(FromRow, Serialize, Debug)]
pub struct BookingRow {
    person_email: String,
    session_datetime: DateTime<Utc>,
    session_location_name: Option<String>,
    session_trainer_email: Option<String>
}

#[derive(Serialize, Debug)]
pub struct AllTables {
    session_type: Vec<SessionTypeRow>,
    location: Vec<LocationRow>,
    person: Vec<PersonRow>,
    session: Vec<SessionRow>,
    booking: Vec<BookingRow>
}

#[get("/backup")]
pub async fn backup_all(pool: &State<PgPool>, claim: Claims) -> Result<Json<AllTables>, Custom<String>> {
    claim.assert_roles_contains("admin")?;
    Ok(Json(AllTables{
        session_type: session_type_table(pool).await?,
        location: location_table(pool).await?,
        person: person_table(pool).await?,
        session: session_table(pool).await?,
        booking: booking_table(pool).await?
    }))
}


async fn person_table(pool: &PgPool) -> Result<Vec<PersonRow>, Custom<String>> {
    query_as("SELECT * FROM person")
        .fetch_all(pool)
        .await
        .map_err(|e| Custom(Status::InternalServerError, format!("person: {}", e)))
}

async fn session_type_table(pool: &PgPool) -> Result<Vec<SessionTypeRow>, Custom<String>> {
    query_as("SELECT * FROM session_type")
        .fetch_all(pool)
        .await
        .map_err(|e| Custom(Status::InternalServerError, format!("session_type: {}", e)))
}

async fn location_table(pool: &PgPool) -> Result<Vec<LocationRow>, Custom<String>> {
    query_as("SELECT * FROM location")
        .fetch_all(pool)
        .await
        .map_err(|e| Custom(Status::InternalServerError, format!("location: {}", e)))
}

async fn session_table(pool: &PgPool) -> Result<Vec<SessionRow>, Custom<String>> {
    query_as("SELECT s.id, s.datetime, s.duration_mins, s.max_booking_count AS max_booking_count, s.notes AS notes, s.cost AS cost, st.name AS session_type_name, l.name AS location_name, t.email AS trainer_email \
            FROM session AS s \
            JOIN session_type AS st ON s.session_type = st.id \
            LEFT JOIN location AS l ON s.location = l.id \
            LEFT JOIN person AS t ON s.trainer = t.id")
        .fetch_all(pool)
        .await
        .map_err(|e| Custom(Status::InternalServerError, format!("session: {}", e)))
}

async fn booking_table(pool: &PgPool) -> Result<Vec<BookingRow>, Custom<String>> {
    query_as("SELECT p.email AS person_email, s.datetime AS session_datetime, l.name AS session_location_name, t.email AS session_trainer_email \
            FROM booking as b \
            JOIN person AS p ON b.person_id = p.id \
            JOIN session AS s ON b.session_id = s.id \
            LEFT JOIN location AS l ON s.location = l.id \
            LEFT JOIN person AS t ON s.trainer = t.id")
        .fetch_all(pool)
        .await
        .map_err(|e| Custom(Status::InternalServerError, format!("booking: {}", e)))
}

#[cfg(test)]
mod tests {
    use chrono::Duration;
    use rocket::serde::json::serde_json;
    use rocket::State;
    use sqlx::{Executor, FromRow, PgPool, query_as};
    use crate::claims::Claims;

    #[derive(FromRow)]
    struct IntRecord {
        id: i32
    }
    #[derive(FromRow)]
    struct BigintRecord {
        id: i64
    }

    #[sqlx::test]
    async fn backup_all(pool: PgPool) {
        pool.execute(include_str!("../schema.sql")).await.unwrap();

        let person_id: BigintRecord = query_as("INSERT INTO person (name, email, phone, pwd, roles, credits) VALUES ('Mr Test', 'test@example.com', '0111', '', 'member', 0) RETURNING id")
            .fetch_one(&pool)
            .await.unwrap();
        let session_type_id: IntRecord = query_as("SELECT id FROM session_type WHERE name = 'HIIT'")
            .fetch_one(&pool)
            .await.unwrap();
        let location_id: IntRecord = query_as("SELECT id FROM location WHERE name = 'Oak Hill Park'")
            .fetch_one(&pool)
            .await.unwrap();
        let session_id: BigintRecord = query_as("INSERT INTO session (datetime, duration_mins, session_type, location, trainer, max_booking_count, notes, cost) VALUES ('2024-01-01 08:00:00Z', 60, $1, $2, null, null, null, 1) RETURNING id")
            .bind(session_type_id.id)
            .bind(location_id.id)
            .fetch_one(&pool)
            .await.unwrap();
        let _booking_session_id: BigintRecord = query_as("INSERT INTO booking (person_id, session_id, credits_used) VALUES ($1, $2, 1) RETURNING session_id AS id")
            .bind(person_id.id)
            .bind(session_id.id)
            .fetch_one(&pool)
            .await.unwrap();

        let claim = Claims::create(0, "", "admin@example.com", &Some("011111".to_string()), &vec!["admin".to_string()], Duration::minutes(1));
        let backup_result = crate::backup::backup_all(State::from(&pool), claim).await.unwrap().into_inner();

        assert_eq!("{\"session_type\":[{\"id\":1,\"name\":\"HIIT\",\"requires_trainer\":true,\"cost\":1,\"deprecated\":false},{\"id\":2,\"name\":\"Strong\",\"requires_trainer\":true,\"cost\":1,\"deprecated\":false},{\"id\":3,\"name\":\"On The Move\",\"requires_trainer\":true,\"cost\":1,\"deprecated\":false}],\"location\":[{\"id\":1,\"name\":\"Oak Hill Park\",\"address\":\"Oak Hill Park, Parkside Gardens, London EN4 8JP\",\"url\":null},{\"id\":2,\"name\":\"Trent Park\",\"address\":\"Trent Park, London EN4 0PS\",\"url\":null}],\"person\":[{\"id\":1,\"name\":\"Mr Test\",\"email\":\"test@example.com\",\"phone\":\"0111\",\"pwd\":\"\",\"roles\":\"member\",\"credits\":0}],\"session\":[{\"id\":1,\"datetime\":\"2024-01-01T08:00:00Z\",\"duration_mins\":60,\"session_type_name\":\"HIIT\",\"location_name\":\"Oak Hill Park\",\"trainer_email\":null,\"max_booking_count\":null,\"notes\":null,\"cost\":1}],\"booking\":[{\"person_email\":\"test@example.com\",\"session_datetime\":\"2024-01-01T08:00:00Z\",\"session_location_name\":\"Oak Hill Park\",\"session_trainer_email\":null}]}", serde_json::to_string(&backup_result).unwrap());
    }
}