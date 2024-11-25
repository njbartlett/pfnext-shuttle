use chrono::{Days, Utc};
use sqlx::{PgPool, query_scalar};

use crate::loginsession::LoginSession;

const DUMMY_SESSION_ID: &str = "xxx";

pub async fn find_user_login_by_name(pool: &PgPool, name: &str, role: &str) -> LoginSession {

    use crate::loginsession::Roles;

    let uid = find_person_id_by_name(pool, name).await;
    LoginSession {
        sessionid: DUMMY_SESSION_ID.to_string(),
        uid,
        name: name.to_string(),
        email: format!("{}@example.com", name),
        roles: Roles::parse(role),
        loggedin: None, loggedin_from: None,
        expiry: Utc::now().checked_add_days(Days::new(1)).unwrap().fixed_offset()
    }
}

pub async fn find_person_id_by_name(pool: &PgPool, name: &str) -> i64 {
    query_scalar("SELECT id FROM person WHERE name = $1")
        .bind(name)
        .fetch_one(pool)
        .await
        .expect(&format!("failed to find user with name {}", name))
}
