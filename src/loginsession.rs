
use mail_send::smtp::client;
use rocket_client_addr::ClientRealAddr;
use ::time::OffsetDateTime;
use user_agent_parser::UserAgent;
use std::{fmt::{Display, Formatter}};

use base64::{prelude::{BASE64_STANDARD_NO_PAD}, Engine};
use chrono::{DateTime, Duration, FixedOffset, Offset};

#[cfg(test)]
use crate::mock_chrono::Utc;
#[cfg(not(test))]
use chrono::Utc;

use password_auth::verify_password;
use rand::{thread_rng, RngCore};
use rocket::{
    http::{private::cookie::Expiration, Cookie, CookieJar, SameSite, Status},
    request::{FromRequest, Outcome},
    response::status::{Custom, NoContent},
    serde::json::{self, Json, json},
    Request, State};
use serde::{Deserialize, Serialize};
use sqlx::{postgres::PgRow, query, query_as, FromRow, PgPool, Row};

use crate::{log, users::UserLoginRecord};

const SESSION_ID: &str = "sessionid";
const ADMIN: &str = "admin";
const SESSION_DURATION: Duration = Duration::days(7);

#[derive(Debug, PartialEq, Clone)]
pub(crate) enum AuthenticationError {
    MissingSession,
    MissingDatabase,
    DatabaseError(String)
}

#[derive(Debug, PartialEq, Clone)]
pub(crate) enum LoginError {
    InvalidLogin,
    ResetRequired,
    Internal(String)
}

impl Display for LoginError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            LoginError::InvalidLogin => f.write_str("Incorrect user email or password"),
            LoginError::ResetRequired => f.write_str("You are required to reset your password"),
            LoginError::Internal(msg) => write!(f, "Server error: {}", msg),
        }
    }
}

#[derive(Serialize, Clone, Debug)]
pub(crate) struct LoginSession {
    pub(crate) sessionid: String,
    pub(crate) uid: i64,
    pub(crate) name: String,
    pub(crate) email: String,
    pub(crate) roles: Vec<String>,
    pub(crate) expiry: DateTime<FixedOffset>,
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for LoginSession {

    type Error = AuthenticationError;

    async fn from_request(request: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        // Get the session cookie
        let sessionid: Option<String> = request.cookies()
            .get_private(SESSION_ID)
            .map(|c| c.value().to_string());

        // No cookie in request header => browser has expired the session
        if sessionid.is_none() {
            return Self::set_outcome_error(AuthenticationError::MissingSession, request);
        }
        let sessionid = sessionid.unwrap();

        // Get the database pool from the request state
        let pool: Option<&PgPool> = request.rocket().state();
        if pool.is_none() {
            return Self::set_outcome_error(AuthenticationError::MissingDatabase, request);
        }
        let pool = pool.unwrap();

        // Load the session record
        let load_result = LoginSession::load(&pool, &sessionid).await
            .inspect_err(|e| error!("Failed to query login session: {}", e));
        if load_result.is_err() {
            return Self::set_outcome_error(AuthenticationError::DatabaseError(load_result.unwrap_err().to_string()), request);
        }
        let login_session = load_result.unwrap();

        // No session record in db => we have expired the session
        if login_session.is_none() {
            return Self::set_outcome_error(AuthenticationError::MissingSession, request);
        }

        debug!("Authenticated user session {:?}", login_session);
        Outcome::Success(login_session.unwrap())
    }

}

impl FromRow<'_, PgRow> for LoginSession {
    fn from_row(row: &'_ PgRow) -> Result<Self, sqlx::Error> {
        Ok(LoginSession {
            sessionid: row.try_get("sessionid")?,
            uid: row.try_get("uid")?,
            name: row.try_get("name")?,
            email: row.try_get("email")?,
            roles: row.try_get("roles").map(|r| parse_roles(r))?,
            expiry: row.try_get("expiry")?
        })
    }
}

impl LoginSession {

    fn set_outcome_error(e: AuthenticationError, request: &'_ Request<'_>) -> Outcome<Self, AuthenticationError> {
        request.local_cache::<Option<AuthenticationError>, _>(|| Some(e.clone()));
        Outcome::Error((Status::Unauthorized, e))
    }

    pub(crate) async fn load(pool: &PgPool, sessionid: &str) -> Result<Option<LoginSession>, sqlx::Error> {
        let now = Utc::now().fixed_offset();
        query_as("SELECT s.id AS sessionid, s.expiry, p.id AS uid, p.name, p.email, p.roles \
                FROM loginsession AS s \
                JOIN person AS p ON s.uid = p.id \
                WHERE s.id = $1 \
                AND s.expiry >= $2")
            .bind(sessionid).bind(now)
            .fetch_optional(pool)
            .await
    }

    pub(crate) async fn delete(&self, pool: &PgPool) -> Result<bool, sqlx::Error> {
        let result = query("DELETE FROM loginsession WHERE id = $1 RETURNING id")
            .bind(self.sessionid.clone())
            .execute(pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    pub(crate) async fn login(pool: &PgPool, email: &str, password: &str, ipinfo: &Option<String>) -> Result<LoginSession, LoginError> {
        Self::clear_expired(pool, &Utc::now().fixed_offset()).await;

        // Fetch login record
        let login_record = UserLoginRecord::load_by_email(pool, email)
            .await
            .map_err(|e| LoginError::Internal(e.to_string()))?;
        if login_record.is_none() {
            return Err(LoginError::InvalidLogin);
        }

        // Verify login
        let login_record = login_record.unwrap();
        if login_record.pwd.is_none() {
            return Err(LoginError::ResetRequired);
        }
        verify_password(password, &login_record.pwd.unwrap())
            .map_err(|_| LoginError::InvalidLogin)?;

        // Generate session ID
        let mut bytes = [0u8; 32];
        thread_rng().fill_bytes(&mut bytes);
        let sessionid = BASE64_STANDARD_NO_PAD.encode(bytes);

        // Insert session into table
        let expiry = Utc::now() + SESSION_DURATION;
        query(
            "INSERT INTO loginsession (id, uid, expiry, ipinfo) VALUES ($1, $2, $3, $4) RETURNING id")
            .bind(&sessionid).bind(login_record.id).bind(expiry).bind(ipinfo)
            .fetch_one(pool)
            .await
            .map_err(|e| LoginError::Internal(e.to_string()))?;
        
        // Return login session struct
        Ok(LoginSession {
            sessionid,
            uid: login_record.id,
            name: login_record.name,
            email: login_record.email,
            roles: parse_roles(&login_record.roles),
            expiry: expiry.fixed_offset()
        })
    }

    pub(crate) async fn relogin(&self, pool: &PgPool, password: &str, ipinfo: &Option<String>) -> Result<LoginSession, LoginError> {
        // Fetch login record
        let login_record = UserLoginRecord::load_by_email(pool, &self.email)
            .await
            .map_err(|e| LoginError::Internal(e.to_string()))?;
        if login_record.is_none() {
            return Err(LoginError::InvalidLogin);
        }

        // Verify login
        let login_record = login_record.unwrap();
        if login_record.pwd.is_none() {
            return Err(LoginError::ResetRequired);
        }
        if let Err(err) = verify_password(password, &login_record.pwd.unwrap()) {
            error!("Failed to verify password on re-login for {}, removing session. {}", self.email, err);
            self.delete(pool).await.map_err(|e| LoginError::Internal(e.to_string()))?;
            return Err(LoginError::InvalidLogin);
        }

        // Update session in table
        let expiry = (Utc::now() + SESSION_DURATION).fixed_offset();
        let _update_result = query("UPDATE loginsession SET expiry = $1, ipinfo = $2 WHERE id = $3")
            .bind(expiry).bind(ipinfo).bind(&self.sessionid)
            .execute(pool)
            .await
            .map_err(|e| LoginError::Internal(e.to_string()))?;

        Ok(LoginSession {
            sessionid: self.sessionid.clone(),
            uid: self.uid,
            email: self.email.clone(),
            name: self.name.clone(),
            roles: self.roles.clone(),
            expiry
        })
    }

    pub(crate) fn has_role(&self, required_role: &str) -> bool {
        self.roles.iter().any(|r| r == required_role)
    }

    pub(crate) fn is_admin(&self) -> bool {
        self.has_role(ADMIN)
    }

    async fn clear_expired(pool: &PgPool, now: &DateTime<FixedOffset>) {
        let expired: Result<Vec<String>, sqlx::Error> = query("DELETE FROM loginsession WHERE expiry < $1 RETURNING id")
            .bind(now)
            .fetch_all(pool)
            .await
            .map(|rows| rows.iter().map(|r| r.try_get("id").unwrap_or("<missing id>").to_string()).collect());
        match expired {
            Ok(expired) => info!("Deleted {} expired sessions: {:?}", expired.len(), expired),
            Err(e) => error!("Failed to delete expired sessions: {}", e),
        };
    }


}

fn parse_roles(roles_str: &str) -> Vec<String> {
    let parsed_roles = roles_str
        .split(",")
        .map(|s| s.to_string())
        .collect::<Vec<_>>();

    if vec![String::from("")].eq(&parsed_roles) {
        Vec::new()
    } else {
        parsed_roles
    }
}

async fn log_login(pool: &PgPool, email: &String, ipinfo: &Option<String>, success: bool) {
    let log_detail = format!("email={}, ipinfo={}", email, ipinfo.as_ref().unwrap_or(&"None".to_string()));
    let event_type = match success {
        true => "LOGIN",
        false => "FAILED LOGIN",
    };
    let _ = log::append_log(pool, &None, event_type, &log_detail).await;
}

#[derive(Deserialize)]
pub struct LoginRequest {
    email: String,
    password: String,
}

#[derive(Serialize)]
pub struct LoggedInUser {
    id: i64,
    name: String,
    email: String,
    roles: Vec<String>
}

#[post("/login", data = "<login_request>")]
pub async fn login(
    pool: &State<PgPool>,
    cookies: &CookieJar<'_>,
    existing_login: Option<LoginSession>,
    client_addr: Option<ClientRealAddr>,
    login_request: Json<LoginRequest>
) -> Result<Json<LoggedInUser>, Custom<String>> {
    let mut ipinfo: Option<String> = None;
    if let Some(client_addr) = client_addr {
        let client_ipv6_addr = client_addr.get_ipv6();       
        if let Ok(lookup_result) = public_ip_address::perform_lookup(Some(client_ipv6_addr.to_canonical())).await {
            info!("Login attempt for user <{}> from {:?}", login_request.email, lookup_result);
            ipinfo = Some(json::to_string(&lookup_result).unwrap_or("{}".to_string()));
        }
    }

    let login_result = match existing_login {
        Some(existing_login) => existing_login.relogin(pool, &login_request.password, &ipinfo).await.map_err(to_http_err),
        None => LoginSession::login(pool, &login_request.email, &login_request.password, &ipinfo).await.map_err(to_http_err)
    };
    if let Err(login_err) = login_result {
        // Log the failed login
        log_login(pool, &login_request.email, &ipinfo, false).await;

        // Send the cookie with sessionid set to "_" and Max-Age zero, to ensure the cookie is removed from the browser
        let mut clear_cookie = Cookie::new(SESSION_ID, "_".to_string());
        clear_cookie.set_max_age(Some(rocket::time::Duration::ZERO));
        cookies.add(clear_cookie);

        return Err(login_err);
    }
    let login_session = login_result.unwrap();

    // Log the successful login
    log_login(pool, &login_request.email, &ipinfo, true).await;

    // Build login response body
    let body = LoggedInUser {
        id: login_session.uid,
        name: login_session.name,
        email: login_session.email,
        roles: login_session.roles
    };
    
    // Add session cookie
    let mut cookie = Cookie::new(SESSION_ID, login_session.sessionid);
    let expiry_chrono = Utc::now() + SESSION_DURATION;
    let expiry_offset_datetime: OffsetDateTime = OffsetDateTime::from_unix_timestamp(expiry_chrono.timestamp())
        .map_err(|e| Custom(Status::InternalServerError, e.to_string()))?;

    cookie.set_secure(true);
    cookie.set_same_site(SameSite::None);
    cookie.unset_domain();
    cookie.set_http_only(true);
    cookie.set_expires(Expiration::DateTime(expiry_offset_datetime));
    cookies.add_private(cookie);

    Ok(Json(body))
}

#[post("/logout")]
pub async fn logout(
    pool: &State<PgPool>,
    login: Option<LoginSession>,
    cookies: &CookieJar<'_>
) -> NoContent {
    // If there is a loginsession associated with a cookie, delete it
    if let Some(login) = login {
        match login.delete(pool.inner()).await {
            Ok(true) => info!("Deleted session record {:?}", login),
            Ok(false) => info!("No session record to delete for {:?}", login),
            Err(e) => error!("Failed to delete session record {:?}: {}", login, e)
        };
    }

    // Remove the session from the cookies, clearing it from the browser
    let mut cookie = Cookie::new(SESSION_ID, "_" );
    cookie.set_max_age(Some(rocket::time::Duration::ZERO));
    cookies.add(cookie);
    NoContent
}

#[get("/verify_session")]
pub async fn verify_session(
    _login: LoginSession
) -> Result<NoContent, Custom<String>> {
    Ok(NoContent)
}    

fn to_http_err(e: LoginError) -> Custom<String> {
    info!("Converting LoginError {:?} to Custom<String>", e);
    let status = match e {
        LoginError::Internal(_) => Status::InternalServerError,
        _ => Status::Unauthorized
    };
    Custom(status, e.to_string())
}

#[cfg(test)]
mod tests {
    use rocket::{http::Status, local::asynchronous::{Client, LocalResponse}, serde::json::json, Build,  Rocket};
    use sqlx::{query, Executor, PgPool, Row};

    use crate::{loginsession::{LoginError, LoginSession}, mock_chrono::set_timestamp_rfc3339};

    #[sqlx::test(fixtures("../schema.sql", "fixtures/users.sql"))]
    async fn test_load_session(pool: PgPool) {
        pool.execute("INSERT INTO loginsession (id, uid, expiry) SELECT 'xxx', id, '2030-01-01T00:00:00Z' FROM person WHERE name = 'user1'").await.expect("failed to create session");
        let session = LoginSession::load(&pool, "xxx").await.unwrap().unwrap();

        assert_eq!("user1", session.name);
    }

    #[sqlx::test(fixtures("../schema.sql", "fixtures/users.sql"))]
    async fn test_load_session_missing(pool: PgPool) {
        let opt_session = LoginSession::load(&pool, "xxx").await.unwrap();
        assert!(opt_session.is_none(), "loginsession record should not be found");
    }

    #[sqlx::test(fixtures("../schema.sql", "fixtures/users.sql"))]
    async fn test_load_session_expired(pool: PgPool) {
        // Insert a session that has already expired
        pool.execute("INSERT INTO loginsession (id, uid, expiry) SELECT 'xxx', id, '2030-01-01T00:00:00Z' FROM person WHERE name = 'user1'").await.expect("failed to create session");
        set_timestamp_rfc3339("2030-01-01T00:00:30Z");

        let opt_session = LoginSession::load(&pool, "xxx").await.unwrap();
        assert!(opt_session.is_none(), "loginsession record should not be found");
    }

    #[sqlx::test(fixtures("../schema.sql", "fixtures/users.sql"))]
    async fn test_login_deletes_expired_unrelated_sessions(pool: PgPool) {
        // Insert a session that has already expired
        pool.execute("INSERT INTO loginsession (id, uid, expiry) SELECT 'xxx', id, '2030-01-01T00:00:00Z' FROM person WHERE name = 'user1'").await.expect("failed to create session");
        assert_eq!(1, query("SELECT * FROM loginsession WHERE id = 'xxx'").execute(&pool).await.unwrap().rows_affected());
        set_timestamp_rfc3339("2030-01-01T00:00:30Z");

        // Try a login => expired session is deleted
        let login = LoginSession::login(&pool, "user1@example.com", "password", &None).await.unwrap();
        assert_eq!(1, count_session_rows(&pool).await);
        assert_eq!(0, query("SELECT * FROM loginsession WHERE id = 'xxx'").execute(&pool).await.unwrap().rows_affected());
    }

    #[sqlx::test(fixtures("../schema.sql", "fixtures/users.sql"))]
    async fn test_login_incorrect_user(pool: PgPool) {
        let err = LoginSession::login(&pool, "missing@example.com", "password", &None).await.unwrap_err();
        assert_eq!(LoginError::InvalidLogin, err);
    }

    #[sqlx::test(fixtures("../schema.sql", "fixtures/users.sql"))]
    async fn test_login_incorrect_password(pool: PgPool) {
        let err = LoginSession::login(&pool, "user1@example.com", "wrong", &None).await.unwrap_err();
        assert_eq!(LoginError::InvalidLogin, err);
    }

    #[sqlx::test(fixtures("../schema.sql", "fixtures/users.sql"))]
    async fn test_login_session_created(pool: PgPool) {
        let new_session = LoginSession::login(&pool, "user1@example.com", "password", &None).await.unwrap();
        assert_eq!("user1", new_session.name);

        let read_session = LoginSession::load(&pool, &new_session.sessionid).await.unwrap().unwrap();
        assert_eq!(new_session.sessionid, read_session.sessionid);
        assert_eq!("user1", read_session.name);
    }

    pub(crate) async fn count_session_rows(pool: &PgPool) -> i64 {
        let count_record = query("SELECT COUNT(*) AS c FROM loginsession")
            .fetch_one(pool)
            .await.unwrap();
        count_record.try_get("c").unwrap()
    }

    fn rocket(pool: PgPool) -> Rocket<Build> {
        rocket::build()
            .manage(pool)
            .mount("/", routes![
                crate::loginsession::login,
                crate::loginsession::verify_session,
                crate::loginsession::logout,
            ])
    }
    
    #[sqlx::test(fixtures("../schema.sql", "fixtures/users.sql"))]
    fn dispatch_login_then_verify_then_logout(pool: PgPool) {        
        let client = Client::untracked(rocket(pool.clone())).await.expect("valid rocket instance");
    
        assert_eq!(count_session_rows(&pool).await, 0, "should be 0 session initially");
    
        // Login
        let login = dispatch_login(&client, "user1@example.com", "password").await.unwrap();
        assert_eq!(count_session_rows(&pool).await, 1, "should be 1 session after login");
    
        // Verify
        let resp_verify_login = client.get(uri!(crate::loginsession::verify_session))
            .cookie(("sessionid", login.sessionid_raw.clone()))
            .dispatch().await;
        assert_eq!(resp_verify_login.status(), Status::NoContent);
        assert_eq!(count_session_rows(&pool).await, 1, "should be 1 session after login/verify");
    
        // Logout
        let resp_logout = client.post(uri!(crate::loginsession::logout))
            .cookie(("sessionid", login.sessionid_raw.clone()))
            .dispatch().await;
        assert_eq!(resp_logout.status(), Status::NoContent);
        assert_eq!(count_session_rows(&pool).await, 0, "should be 0 sessions after logout");
        assert_eq!(resp_logout.cookies().get("sessionid").unwrap().max_age().unwrap().whole_microseconds(), 0);
    }

    #[sqlx::test(fixtures("../schema.sql", "fixtures/users.sql"))]
    fn dispatch_login_then_login_again_extends_session_expiry(pool: PgPool) {        
        let client = Client::untracked(rocket(pool.clone())).await.expect("valid rocket instance");
    
        assert_eq!(count_session_rows(&pool).await, 0, "should be 0 session initially");

        // Set initial time to 00:00 on 1 Jan 2025 UTC
        set_timestamp_rfc3339("2025-01-01T00:00:00Z");
    
        // Login
        let login1 = dispatch_login(&client, "user1@example.com", "password").await.unwrap();
        assert_eq!(count_session_rows(&pool).await, 1, "should be 1 session after login");
        assert_ne!(-1, login1.expiry);

        // Advance clock by 1 hour
        set_timestamp_rfc3339("2025-01-01T01:00:00Z");

        // Login again with existing cookie => no new session record, existing session is extended
        let mut post = client.post(uri!(crate::loginsession::login))
            .cookie(("sessionid", login1.sessionid_raw.clone()));
        post.set_body(json!({
            "email": "user1@example.com",
            "password": "password"
        }).to_string());

        let resp = post.dispatch().await;
        assert_eq!(resp.status(), Status::Ok);
        assert_eq!(count_session_rows(&pool).await, 1, "should still be 1 session after login");
        let login2 = LoginCookie::from_response(&resp).unwrap();
        
        assert_eq!(login1.sessionid, login2.sessionid);
        let expiry_delta = login2.expiry - login1.expiry;
        assert_eq!(expiry_delta, 3600, "cookie expiry should be extended by 1 hour");
    }
    
    #[sqlx::test(fixtures("../schema.sql", "fixtures/users.sql"))]
    fn dispatch_login_then_login_with_incorrect_password_deletes_session(pool: PgPool) {        
        let client = Client::untracked(rocket(pool.clone())).await.expect("valid rocket instance");
    
        assert_eq!(count_session_rows(&pool).await, 0, "should be 0 session initially");
    
        // Login
        let login1 = dispatch_login(&client, "user1@example.com", "password").await.unwrap();
        assert_eq!(count_session_rows(&pool).await, 1, "should be 1 session after login");
        assert_ne!(-1, login1.expiry);

        // Login again with existing cookie but wrong password => deletes session record
        let mut post = client.post(uri!(crate::loginsession::login))
            .cookie(("sessionid", login1.sessionid_raw.clone()));
        post.set_body(json!({
            "email": "user1@example.com",
            "password": "wrong"
        }).to_string());

        let resp = post.dispatch().await;
        info!("{:?}", resp);
        assert_eq!(resp.status(), Status::Unauthorized);
        assert_eq!(count_session_rows(&pool).await, 0, "should be 0 sessions after incorrect login");
        let login2 = LoginCookie::from_response(&resp).unwrap();
        assert_eq!(login2.sessionid, "_".to_string(), "sessionid should be cleared from cookie");
        assert_eq!(login2.max_age_secs, 0, "session cookie should have max_age zeroed to delete it from the browser");
    }

    #[sqlx::test(fixtures("../schema.sql", "fixtures/users.sql"))]
    fn dispatch_login_then_login_again_without_cookie_creates_new_session(pool: PgPool) {        
        let client = Client::untracked(rocket(pool.clone())).await.expect("valid rocket instance");
    
        assert_eq!(count_session_rows(&pool).await, 0, "should be 0 session initially");
    
        // Login
        let login1 = dispatch_login(&client, "user1@example.com", "password").await.unwrap();
        assert_eq!(count_session_rows(&pool).await, 1, "should be 1 session after login");

        // Login again without cookie => no new session record, existing session is extended, no
        let login2 = dispatch_login(&client, "user1@example.com", "password").await.unwrap();
        assert_eq!(count_session_rows(&pool).await, 2, "should be 2 sessions after second login without cookie");
        
        assert_ne!(login1.sessionid, login2.sessionid);
    }

    #[derive(Debug, PartialEq)]
    struct LoginCookie {
        sessionid: String,
        sessionid_raw: String,
        max_age_secs: i64,
        expiry: i64,
    }

    impl LoginCookie {
        fn from_response(resp: &LocalResponse) -> Option<Self> {
            resp.cookies().get("sessionid").map(|cookie| {
                let cookie_private = resp.cookies()
                    .get_private("sessionid")
                    .map(|c| c.value().to_string())
                    .unwrap_or_else(|| cookie.value().to_string());
                Self {
                    sessionid_raw: cookie.value().to_string(),
                    sessionid: cookie_private,
                    max_age_secs: cookie.max_age()
                        .map(|d| d.whole_seconds())
                        .unwrap_or(-1),
                    expiry: cookie.expires_datetime()
                        .map(|dt| dt.to_utc().unix_timestamp())
                        .unwrap_or(-1)
                }
            })
        }
    }

    async fn dispatch_login(client: &Client, email: &str, password: &str) -> Option<LoginCookie> {
        let mut post = client.post(uri!(crate::loginsession::login));
        let json = json!({
            "email": email,
            "password": password
        }).to_string();
        post.set_body(json);

        info!(">>> Sending login request: {:?}", post);
        let resp = post.dispatch().await;
        if resp.status().code >= 400 {
            error!(">>> Error in login response: {:?}", resp);
        } else {
            info!(">>> Login response: {:?}", resp);
        }

        assert_eq!(resp.status(), Status::Ok);
        LoginCookie::from_response(&resp)
    }

}    
