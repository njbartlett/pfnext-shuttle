use chrono::{DateTime, Utc};
use rocket::http::Status;
use rocket::response::status::Custom;
use rocket::serde::json::Json;
use rocket::serde::Serialize;
use rocket::State;
use sqlx::{FromRow, PgPool, query_as, QueryBuilder};
use crate::{BigintRecord, parse_opt_date};
use crate::claims::Claims;

#[derive(FromRow, Serialize, Debug)]
pub struct LogRow {
    id: i64,
    datetime: DateTime<Utc>,
    person: String,
    event_type: String,
    detail: String
}

pub async fn append_log(pool: &PgPool, claim: &Claims, event_type: String, detail: String) -> Result<i64, Custom<String>> {
    let timestamp: DateTime<Utc> = Utc::now();
    let new_record: BigintRecord = query_as("INSERT INTO eventlog (datetime, person, type, detail) VALUES ($1, $2, $3, $4) RETURNING id")
        .bind(timestamp)
        .bind(&claim.name)
        .bind(event_type)
        .bind(detail)
        .fetch_one(pool)
        .await
        .map_err(|e| Custom(Status::InternalServerError, e.to_string()))?;
    Ok(new_record.id)
}

#[get("/log?<from>&<to>")]
pub async fn read_log(
    pool: &State<PgPool>,
    claim: Claims,
    from: Option<String>,
    to: Option<String>
) -> Result<Json<Vec<LogRow>>, Custom<String>> {
    claim.assert_roles_contains("admin")?;

    let mut qb = QueryBuilder::new("SELECT id, datetime, person, type AS event_type, detail FROM eventlog");

    let mut where_op= String::from(" WHERE");
    if let Some(from) = parse_opt_date(from)? {
        qb.push(where_op + " datetime >= ");
        qb.push_bind(from);
        where_op = String::from(" AND");
    }
    if let Some(to) = parse_opt_date(to)? {
        qb.push(where_op + " datetime <= ");
        qb.push_bind(to);
    }

    qb.push(" ORDER BY datetime DESC");

    qb.build_query_as()
        .fetch_all(pool.inner())
        .await
        .map_err(|e| Custom(Status::InternalServerError, e.to_string()))
        .map(|v| Json(v))
}

#[cfg(test)]
mod tests {
    use chrono::{Duration};
    use rocket::http::Status;
    use rocket::response::status::Custom;
    use rocket::State;
    use sqlx::{Executor, PgPool, query_as};
    use crate::BigintRecord;
    use crate::claims::Claims;

    #[sqlx::test]
    async fn read_empty_not_admin(pool: PgPool) {
        pool.execute(include_str!("../schema.sql")).await.unwrap();

        let claim = Claims::create(0, "", "member@example.com", &Some("0".to_string()), &vec!["member".to_string()], Duration::minutes(1));
        let read_result = crate::log::read_log(State::from(&pool), claim, None, None).await;
        assert_eq!(Custom(Status::Forbidden, "user is not allowed to perform this action".to_string()), read_result.unwrap_err());
    }

    #[sqlx::test]
    async fn read_empty_admin(pool: PgPool) {
        pool.execute(include_str!("../schema.sql")).await.unwrap();

        let claim = Claims::create(0, "", "admin@example.com", &Some("0".to_string()), &vec!["admin".to_string()], Duration::minutes(1));
        let read_result = crate::log::read_log(State::from(&pool), claim, None, None).await.unwrap();
        assert_eq!(0, read_result.len());
    }

    #[sqlx::test]
    async fn read_one(pool: PgPool) {
        pool.execute(include_str!("../schema.sql")).await.unwrap();

        let _new_record: BigintRecord = query_as("INSERT INTO eventlog (datetime, person, type, detail) VALUES ('19700101_00:00:00Z', 'joe', 'Test Event', 'Test event') RETURNING ID")
            .fetch_one(&pool)
            .await
            .unwrap();

        let claim = Claims::create(0, "", "admin@example.com", &Some("0".to_string()), &vec!["admin".to_string()], Duration::minutes(1));
        let read_result = crate::log::read_log(State::from(&pool), claim, None, None).await.unwrap();
        assert_eq!(1, read_result.len());
        assert_eq!("Test event", read_result.get(0).unwrap().detail);
    }

    #[sqlx::test]
    async fn read_with_date_range(pool: PgPool) {
        pool.execute(include_str!("../schema.sql")).await.unwrap();

        let _new_record1: BigintRecord = query_as("INSERT INTO eventlog (datetime, person, type, detail) VALUES ('19700101_00:00:00Z', '', 'Test Event', 'Test event 1') RETURNING ID").fetch_one(&pool).await.unwrap();
        let _new_record2: BigintRecord = query_as("INSERT INTO eventlog (datetime, person, type, detail) VALUES ('19800101_00:00:00Z', '', 'Test Event', 'Test event 2') RETURNING ID").fetch_one(&pool).await.unwrap();
        let _new_record3: BigintRecord = query_as("INSERT INTO eventlog (datetime, person, type, detail) VALUES ('19900101_00:00:00Z', '', 'Test Event', 'Test event 3') RETURNING ID").fetch_one(&pool).await.unwrap();

        let claim = Claims::create(0, "", "admin@example.com", &Some("0".to_string()), &vec!["admin".to_string()], Duration::minutes(1));
        let read_result = crate::log::read_log(
            State::from(&pool), claim,
            Some("1980-01-01T00:00:00Z".to_string()),
            Some("1985-01-01T00:00:00Z".to_string()),
        ).await.unwrap();
        assert_eq!(1, read_result.len());
        assert_eq!("Test event 2", read_result.get(0).unwrap().detail);
    }}