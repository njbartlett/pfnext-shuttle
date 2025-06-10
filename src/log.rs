use crate::{loginsession::LoginSession, parse_opt_date};
use chrono::{DateTime, Utc};
use rocket::http::Status;
use rocket::response::status::Custom;
use rocket::serde::json::Json;
use rocket::serde::Serialize;
use rocket::State;
use sqlx::{query, FromRow, PgPool, QueryBuilder, Row};

#[derive(FromRow, Serialize, Debug)]
pub struct LogRow {
    id: i64,
    datetime: DateTime<Utc>,
    person: String,
    event_type: String,
    detail: String
}

pub async fn append_log(pool: &PgPool, originator: &Option<String>, event_type: &str, detail: &str) -> Result<i64, Custom<String>> {
    let timestamp: DateTime<Utc> = Utc::now();
    query("INSERT INTO eventlog (datetime, person, type, detail) VALUES ($1, $2, $3, $4) RETURNING id")
        .bind(timestamp)
        .bind(originator.as_ref().map(|s| s.as_str()).unwrap_or("<missing>"))
        .bind(event_type)
        .bind(detail)
        .fetch_one(pool)
        .await
        .and_then(|r| r.try_get("id"))
        .map_err(|e| Custom(Status::InternalServerError, e.to_string()))
}

#[get("/log?<from>&<to>")]
pub async fn read_log(
    pool: &State<PgPool>,
    login: LoginSession,
    from: Option<String>,
    to: Option<String>
) -> Result<Json<Vec<LogRow>>, Custom<String>> {
    if !login.is_admin() {
        return Err(Custom(Status::Forbidden, "admin role required to read log".to_string()));
    }

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
        .map(Json::from)
}

#[cfg(test)]
mod tests {
    use chrono::{DateTime, Days, Duration, Utc};
    use rocket::{http::Status, response::status::Custom, State};
    use sqlx::{query, query_as, Executor, PgPool, Row};
    use crate::loginsession::LoginSession;

    const DUMMY_SESSION_ID: &str = "xxx";

    fn create_login(name: &str, role: &str) -> LoginSession {
        LoginSession {
            sessionid: DUMMY_SESSION_ID.to_string(),
            uid: -1,
            name: name.to_string(),
            email: format!("{}@example.com", name),
            roles: vec![role.to_string()],
            expiry: Utc::now().checked_add_days(Days::new(1)).unwrap().fixed_offset()
        }
    }

    #[sqlx::test]
    async fn read_empty_not_admin(pool: PgPool) {
        pool.execute(include_str!("../schema.sql")).await.unwrap();

        let login = create_login("member", "member");
        let read_result = crate::log::read_log(State::from(&pool), login, None, None).await;
        assert_eq!(Custom(Status::Forbidden, "admin role required to read log".to_string()), read_result.unwrap_err());
    }
    #[sqlx::test]
    async fn read_empty_admin(pool: PgPool) {
        pool.execute(include_str!("../schema.sql")).await.unwrap();

        let login = create_login("admin", "admin");
        let read_result = crate::log::read_log(State::from(&pool), login, None, None).await.unwrap();
        assert_eq!(0, read_result.len());
    }

    #[sqlx::test]
    async fn read_one(pool: PgPool) {
        pool.execute(include_str!("../schema.sql")).await.unwrap();

        let _ = query("INSERT INTO eventlog (datetime, person, type, detail) VALUES ('19700101_00:00:00Z', 'joe', 'Test Event', 'Test event') RETURNING ID")
            .execute(&pool)
            .await;

        let login = create_login("admin", "admin");
        let read_result = crate::log::read_log(State::from(&pool), login, None, None).await.unwrap();
        assert_eq!(1, read_result.len());
        assert_eq!("Test event", read_result.get(0).unwrap().detail);
    }

    #[sqlx::test]
    async fn read_with_date_range(pool: PgPool) {
        pool.execute(include_str!("../schema.sql")).await.unwrap();

        let _new_record1 = insert_log(&pool, "1970-01-01T00:00:00Z", "Test Event", "Test event 1").await;
        let _new_record2 = insert_log(&pool, "1980-01-01T00:00:00Z", "Test Event", "Test event 2").await;
        let _new_record3 = insert_log(&pool, "1990-01-01T00:00:00Z", "Test Event", "Test event 3").await;

        let login = create_login("admin", "admin");
        let read_result = crate::log::read_log(
            State::from(&pool), login,
            Some("1980-01-01T00:00:00Z".to_string()),
            Some("1985-01-01T00:00:00Z".to_string()),
        ).await.unwrap();
        assert_eq!(1, read_result.len());
        assert_eq!("Test event 2", read_result.get(0).unwrap().detail);
    }

    async fn insert_log(pool: &PgPool, timestamp_str: &str, event_type: &str, event_detail: &str) -> i64 {
        let timestamp = DateTime::parse_from_rfc3339(timestamp_str).unwrap();
        query("INSERT INTO eventlog (datetime, person, type, detail) VALUES ($1, '', $2, $3) RETURNING id")
            .bind(timestamp).bind(event_type).bind(event_detail)
            .fetch_one(pool)
            .await
            .and_then(|r| r.try_get("id"))
            .unwrap()
    }

}