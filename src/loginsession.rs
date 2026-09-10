use rocket_client_addr::ClientRealAddr;
use std::fmt::{Display, Formatter};

use base64::{prelude::BASE64_STANDARD_NO_PAD, Engine};
use chrono::{DateTime, Duration, FixedOffset};

#[cfg(test)]
use crate::mock_chrono::Utc;
#[cfg(not(test))]
use chrono::Utc;

use password_auth::verify_password;
use rand::{thread_rng, RngCore};
use rocket::{
    http::{private::cookie::Expiration, Cookie, CookieJar, SameSite, Status}, request::{FromRequest, Outcome}, response::status::{Custom, NoContent}, serde::json::Json, Request, Route, State
};
use serde::{Deserialize, Serialize};
use sqlx::{postgres::{PgRow, PgTypeInfo}, query, Decode, FromRow, PgPool, Postgres, QueryBuilder, Row, Type};

use crate::{config::AppEnv, transaction_log::append_log, users::UserLoginRecord, whereclause::{Operator, WhereClause}};

const SESSION_ID: &str = "sessionid";
const ADMIN: &str = "admin";
const SESSION_DURATION: Duration = Duration::days(7);

pub fn routes() -> Vec<Route> {
    routes![
        delete_session_by_id,
        get_session_by_id,
        get_sessions,
        login,
        logout,
        verify_session,
    ]
}

#[derive(Debug, PartialEq, Clone)]
pub enum AuthenticationError {
    MissingSession,
    MissingDatabase,
    DatabaseError(String)
}

#[derive(Debug, PartialEq, Clone)]
enum LoginError {
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
#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub struct Roles(Vec<String>);

impl Roles {
    pub fn has_role(&self, required_role: &str) -> bool {
        self.0.iter().any(|r| r == required_role)
    }
    pub fn is_admin(&self) -> bool {
        self.has_role(ADMIN)
    }
    pub fn parse(roles_str: &str) -> Self {
        let parsed_roles: Vec<String> = roles_str
            .split(",")
            .map(|s| s.to_string())
            .collect();
        Self(if vec![String::from("")].eq(&parsed_roles) {
                Vec::new()
            } else {
                parsed_roles
            }
        )
    }
}
impl Into<Vec<String>> for Roles {
    fn into(self) -> Vec<String> {
        self.0
    }
}
impl Decode<'_, Postgres> for Roles {
    fn decode(value: <Postgres as sqlx::Database>::ValueRef<'_>) -> Result<Self, sqlx::error::BoxDynError> {
        Ok(Self::parse(value.as_str()?))
    }
}
impl Type<Postgres> for Roles {
    fn type_info() -> <Postgres as sqlx::Database>::TypeInfo {
        PgTypeInfo::with_name("TEXT")
    }
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub struct LoginSession {
    pub sessionid: String,
    pub uid: i64,
    pub name: String,
    pub email: String,
    pub roles: Roles,
    pub loggedin: Option<DateTime<FixedOffset>>,
    pub loggedin_from: Option<String>,
    pub expiry: DateTime<FixedOffset>
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for LoginSession {

    type Error = AuthenticationError;

    async fn from_request(request: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        // Get the database pool from the request state
        let pool: Option<&PgPool> = request.rocket().state();
        if pool.is_none() {
            return Self::set_outcome_error(AuthenticationError::MissingDatabase, request);
        }
        let pool = pool.unwrap();

        // Get the session cookie
        let sessionid: Option<String> = request.cookies()
            .get_private(SESSION_ID)
            .map(|c| c.value().to_string());

        // No cookie in request header => browser has expired the session
        if sessionid.is_none() {
            info!("Session cookie not sent by client");
            let _ = append_log(pool, &None, "UNAUTHORIZED", "Session cookie not sent by client").await;
            return Self::set_outcome_error(AuthenticationError::MissingSession, request);
        }
        let sessionid = sessionid.unwrap();
        info!("Verifying login session for id {}", sessionid);

        // Load the session record
        let load_result = LoginSession::load(&pool, &sessionid).await;
        if load_result.is_err() {
            return Self::set_outcome_error(AuthenticationError::DatabaseError(load_result.unwrap_err().to_string()), request);
        }
        let login_session = load_result.unwrap();

        // No session record in db => we have expired the session
        if login_session.is_none() {
            info!("Session record not found in table");
            let _ = append_log(pool, &None, "UNAUTHORIZED", "Session record not found in table").await;
            return Self::set_outcome_error(AuthenticationError::MissingSession, request);
        }

        info!("Authenticated user session {:?}", login_session);
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
            roles: row.try_get("roles").map(|r| Roles::parse(r))?,
            loggedin: row.try_get("loggedin")?,
            loggedin_from: row.try_get("loggedin_from")?,
            expiry: row.try_get("expiry")?
        })
    }
}

impl LoginSession {

    fn set_outcome_error(e: AuthenticationError, request: &'_ Request<'_>) -> Outcome<Self, AuthenticationError> {
        request.local_cache::<Option<AuthenticationError>, _>(|| Some(e.clone()));
        Outcome::Error((Status::Unauthorized, e))
    }

    fn query_base<'a>() -> QueryBuilder<'a, Postgres> {
        return QueryBuilder::new("SELECT s.id AS sessionid, s.loggedin, s.loggedin_from, s.expiry, p.id AS uid, p.name, p.email, p.roles FROM loginsession s JOIN person p ON s.uid = p.id");
    }

    async fn load_all(pool: &PgPool) -> Result<Vec<LoginSession>, sqlx::Error> {
        let mut qb = Self::query_base();
        qb.push(" ORDER BY s.loggedin ASC");
        qb.build_query_as()
            .fetch_all(pool)
            .await
    }

    async fn load(pool: &PgPool, sessionid: &str) -> Result<Option<LoginSession>, sqlx::Error> {
        let mut qb = Self::query_base();
        let mut wc = WhereClause::init();
        
        wc.append_to(&mut qb, "s.id", Operator::Equal, sessionid);
        wc.append_to(&mut qb, "s.expiry", Operator::GreaterThanOrEqual, Utc::now().fixed_offset());
        qb.build_query_as()
            .fetch_optional(pool)
            .await
    }

    async fn delete(&self, pool: &PgPool) -> Result<bool, sqlx::Error> {
        let result = query("DELETE FROM loginsession WHERE id = $1 RETURNING id")
            .bind(self.sessionid.clone())
            .execute(pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    async fn login(pool: &PgPool, email: &str, password: &str, ipinfo: &Option<String>) -> Result<LoginSession, LoginError> {
        let now = Utc::now().fixed_offset();
        Self::clear_expired(pool, &now).await;

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
            "INSERT INTO loginsession (id, uid, expiry, loggedin, loggedin_from) VALUES ($1, $2, $3, $4, $5) RETURNING id")
            .bind(&sessionid) // $1
            .bind(login_record.id) // $2
            .bind(expiry) // $4
            .bind(now) // $3
            .bind(ipinfo) // $5
            .fetch_one(pool)
            .await
            .map_err(|e| LoginError::Internal(e.to_string()))?;
        
        // Return login session struct
        Ok(LoginSession {
            sessionid,
            uid: login_record.id,
            name: login_record.name,
            email: login_record.email,
            roles: Roles::parse(&login_record.roles),
            loggedin: Some(now),
            loggedin_from: ipinfo.clone(),
            expiry: expiry.fixed_offset()
        })
    }

    async fn relogin(&self, pool: &PgPool, password: &str) -> Result<LoginSession, LoginError> {
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
        let now = Utc::now().fixed_offset();
        let expiry = now + SESSION_DURATION;
        let _update_result = query("UPDATE loginsession SET expiry = $1 WHERE id = $2")
            .bind(expiry)
            .bind(&self.sessionid)
            .execute(pool)
            .await
            .map_err(|e| LoginError::Internal(e.to_string()))?;

        Ok(LoginSession {
            sessionid: self.sessionid.clone(),
            uid: self.uid,
            email: self.email.clone(),
            name: self.name.clone(),
            roles: self.roles.clone(),
            loggedin: None, loggedin_from: None,
            expiry
        })
    }

    pub fn has_role(&self, required_role: &str) -> bool {
        self.roles.has_role(required_role)
    }

    pub fn is_admin(&self) -> bool {
        self.roles.is_admin()
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

async fn log_login(pool: &PgPool, email: &String, ipinfo: &Option<String>, success: bool) {
    let log_detail = format!("email={}, ipinfo={}", email, ipinfo.as_ref().unwrap_or(&"None".to_string()));
    let event_type = match success {
        true => "LOGIN",
        false => "FAILED LOGIN",
    };
let _ = append_log(pool, &None, event_type, &log_detail).await;
}

#[derive(Deserialize)]
struct LoginRequest {
    email: String,
    password: String,
}

#[derive(Serialize)]
struct LoggedInUser {
    id: i64,
    name: String,
    email: String,
    roles: Vec<String>
}

#[derive(Debug)]
struct ClientInfo<'a> {
    location: String,
    user_agent_product: user_agent_parser::Product<'a>,
    user_agent_os: user_agent_parser::OS<'a>,
    user_agent_device: user_agent_parser::Device<'a>,
    user_agent_cpu: user_agent_parser::CPU<'a>,
    user_agent_engine: user_agent_parser::Engine<'a>
}

const UNKNOWN: &str = "Unknown";

#[rocket::async_trait]
impl<'r> FromRequest<'r> for ClientInfo<'r> {
    
    type Error = ();
    
    async fn from_request(request: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        use user_agent_parser::*;

        let location = Self::extract_ip_info(request).await.unwrap_or(UNKNOWN.to_string());

        let uap = request.rocket().state::<UserAgentParser>();
        let ua = request.headers().get("user-agent").next();
        let info = match (uap, ua) {
            (Some(parser), Some(user_agent)) => ClientInfo {
                location,
                user_agent_product: parser.parse_product(user_agent),
                user_agent_os: parser.parse_os(user_agent),
                user_agent_device: parser.parse_device(user_agent),
                user_agent_cpu: parser.parse_cpu(user_agent),
                user_agent_engine: parser.parse_engine(user_agent)
            },
            _ => ClientInfo {
                location,
                user_agent_product: Product::default(),
                user_agent_os: OS::default(),
                user_agent_device: Device::default(),
                user_agent_cpu: CPU::default(),
                user_agent_engine: Engine::default()
            }
        };
        Outcome::Success(info)
    }
}

impl Display for ClientInfo<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Location: {}", self.location)?;
        
        write!(f, " Product: {}", self.user_agent_product.name.as_deref().unwrap_or(UNKNOWN))?;
        fmt_optional(f, "_", &self.user_agent_product.major)?;
        fmt_optional(f, ".", &self.user_agent_product.minor)?;
        fmt_optional(f, ".", &self.user_agent_product.patch)?;

        write!(f, " OS: {}", self.user_agent_os.name.as_deref().unwrap_or(UNKNOWN))?;
        fmt_optional(f, "_", &self.user_agent_os.major)?;
        fmt_optional(f, ".", &self.user_agent_os.minor)?;
        fmt_optional(f, ".", &self.user_agent_os.patch)?;
        fmt_optional(f, ".", &self.user_agent_os.patch_minor)?;

        fmt_optional(f, " Device-Name: ", &self.user_agent_device.name)?;
        fmt_optional(f, " Device-Brand: ", &self.user_agent_device.brand)?;
        fmt_optional(f, " Device-Model: ", &self.user_agent_device.model)?;

        fmt_optional(f, " CPU: ", &self.user_agent_cpu.architecture)?;

        write!(f, " Engine: {}", self.user_agent_engine.name.as_deref().unwrap_or(UNKNOWN))?;
        fmt_optional(f, "_", &self.user_agent_engine.major)?;
        fmt_optional(f, ".", &self.user_agent_engine.minor)?;
        fmt_optional(f, ".", &self.user_agent_engine.patch)?;

        Ok(())
    }
}

impl ClientInfo<'_> {
    async fn extract_ip_info<'r>(request: &'r Request<'_>) -> Option<String> {
        let client_addr = request.guard::<ClientRealAddr>().await.succeeded();
        match client_addr {
            Some(addr) => {
                let ip = addr.get_ipv6().to_canonical();
                match public_ip_address::perform_lookup(Some(ip)).await {
                    Ok(lookup_result) => Some(lookup_result.to_string()),
                        //.inspect_err(|e| error!("Failed to serialize public IP address lookup {:?}: {}", lookup_result, e))
                        //.ok(),
                    Err(err) => {
                        warn!("Failed to lookup IP info for client {}: {}", ip, err);
                        None
                    }
                }
            },
            None => None
        }
    }
}

fn fmt_optional<T>(f: &mut Formatter<'_>, prefix: &str, opt_value: &Option<T>) -> std::fmt::Result
where
    T: Display
{
    if let Some(value) = opt_value {
        write!(f, "{}{}", prefix, value)
    } else {
        Ok(())
    }
}


#[post("/login", data = "<login_request>")]
async fn login(
    pool: &State<PgPool>,
    app_env: &State<AppEnv>,
    cookies: &CookieJar<'_>,
    existing_login: Option<LoginSession>,
    client_info: Option<ClientInfo<'_>>,
    login_request: Json<LoginRequest>
) -> Result<Json<LoggedInUser>, Custom<String>> {
    let client_info = &client_info.map(|i| i.to_string());

    let login_result = match existing_login {
        Some(existing_login) => existing_login.relogin(pool, &login_request.password).await.map_err(to_http_err),
        None => LoginSession::login(pool, &login_request.email, &login_request.password, &client_info).await.map_err(to_http_err)
    };
    if let Err(login_err) = login_result {
        // Log the failed login
        log_login(pool, &login_request.email, &client_info, false).await;

        // Send the cookie with sessionid set to "_" and Max-Age zero, to ensure the cookie is removed from the browser
        let mut clear_cookie = Cookie::new(SESSION_ID, "_".to_string());
        clear_cookie.set_max_age(Some(rocket::time::Duration::ZERO));
        cookies.add(clear_cookie);

        return Err(login_err);
    }
    let login_session = login_result.unwrap();

    // Log the successful login
    log_login(pool, &login_request.email, &client_info, true).await;

    // Build login response body
    let body = LoggedInUser {
        id: login_session.uid,
        name: login_session.name,
        email: login_session.email,
        roles: login_session.roles.0
    };
    
    // Add session cookie
    let mut cookie = Cookie::new(SESSION_ID, login_session.sessionid);
    cookie.set_secure(app_env.cookie_secure);
    cookie.set_same_site(SameSite::Strict);
    cookie.unset_domain();
    cookie.set_http_only(true);
    let session_duration_secs = SESSION_DURATION.num_seconds();
    cookie.set_max_age(Some(rocket::time::Duration::seconds(session_duration_secs)));
    cookies.add_private(cookie);

    Ok(Json(body))
}

#[post("/logout")]
async fn logout(
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
async fn verify_session(
    _login: LoginSession
) -> Result<NoContent, Custom<String>> {
    Ok(NoContent)
}

#[derive(Serialize)]
struct LoginSessionAugmented {
    uid: i64,
    sessionid: String,
    name: String,
    email: String,
    roles: Roles,
    loggedin: Option<DateTime<FixedOffset>>,
    loggedin_from: Option<String>,
    expiry: DateTime<FixedOffset>,
    is_current: bool
}

impl LoginSessionAugmented {
    fn augment(session: &LoginSession, is_current: bool) -> LoginSessionAugmented {
        LoginSessionAugmented {
            sessionid: session.sessionid.clone(),
            uid: session.uid,
            name: session.name.clone(),
            email: session.email.clone(),
            roles: session.roles.clone(),
            loggedin: session.loggedin.clone(),
            loggedin_from: session.loggedin_from.clone(),
            expiry: session.expiry.clone(),
            is_current
        }
    }
}

#[get("/loginsession")]
async fn get_sessions(
    pool: &State<PgPool>,
    login: LoginSession
) -> Result<Json<Vec<LoginSessionAugmented>>, Custom<String>> {
    if !login.is_admin() {
        return Err(Custom(Status::Forbidden, "admin role required".to_string()));
    }
    LoginSession::load_all(pool)
        .await
        .map_err(|e| Custom(Status::InternalServerError, e.to_string()))
        .map(|v| {
            v.into_iter()
                .map(|l| LoginSessionAugmented::augment(&l, l.sessionid == login.sessionid))
                .collect::<Vec<LoginSessionAugmented>>()
        })
        .map(Json::from)
}

#[get("/loginsession/<id>")]
async fn get_session_by_id(
    pool: &State<PgPool>,
    login: LoginSession,
    id: &str
) -> Result<Json<LoginSessionAugmented>, Custom<String>> {
    if !login.is_admin() {
        return Err(Custom(Status::Forbidden, "admin role required".to_string()));
    }
    let session = LoginSession::load(pool.inner(), id)
        .await
        .map_err(|e| Custom(Status::InternalServerError, e.to_string()))
        .and_then(|o| o.ok_or_else(|| Custom(Status::NotFound, format!("no loginsession id found for id {}", id))))?;
    Ok(Json(LoginSessionAugmented::augment(&session, session.sessionid == login.sessionid)))
}

#[delete("/loginsession/<id>")]
async fn delete_session_by_id(
    pool: &State<PgPool>,
    login: LoginSession,
    id: &str
) -> Result<NoContent, Custom<String>> {
    if !login.is_admin() {
        return Err(Custom(Status::Forbidden, "admin role required".to_string()));
    }
    let result = query("DELETE FROM loginsession WHERE id = $1")
        .bind(id)
        .execute(pool.inner())
        .await
        .map_err(|e| Custom(Status::InternalServerError, e.to_string()))?;
    if result.rows_affected() < 1 {
        Err(Custom(Status::NotFound, "no login session with specified id".to_string()))
    } else {
        Ok(NoContent)
    }
}

fn to_http_err(e: LoginError) -> Custom<String> {
    let status = match e {
        LoginError::Internal(_) => Status::InternalServerError,
        _ => Status::Unauthorized
    };
    Custom(status, e.to_string())
}

#[cfg(test)]
mod tests {
    use chrono::{DateTime};
    use rocket::{http::Status, local::asynchronous::{Client, LocalResponse}, serde::json::json, Build,  Rocket};
    use sqlx::{query, Executor, PgPool, Row};

    use crate::{loginsession::{LoginError, LoginSession, SESSION_ID}, mock_chrono::{set_timestamp_datetime, set_timestamp_rfc3339}};

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
        set_timestamp_rfc3339("2030-01-01T00:00:01Z");

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
        LoginSession::login(&pool, "user1@example.com", "password", &None).await.unwrap();
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
        let now = DateTime::parse_from_rfc3339("2025-01-01T00:00:00Z").unwrap();
        set_timestamp_datetime(&now);

        let new_session = LoginSession::login(&pool, "user1@example.com", "password", &None).await.unwrap();
        assert_eq!("user1", new_session.name);

        let read_session = LoginSession::load(&pool, &new_session.sessionid).await.unwrap().unwrap();
        assert_eq!(new_session.sessionid, read_session.sessionid);
        assert_eq!("user1", read_session.name);
        assert_eq!(Some(now), read_session.loggedin);
        assert_eq!(DateTime::parse_from_rfc3339("2025-01-08T00:00:00Z").unwrap(), read_session.expiry);
    }

    #[sqlx::test(fixtures("../schema.sql", "fixtures/users.sql"))]
    async fn test_login_session_verify(pool: PgPool) {
        let start_time = DateTime::parse_from_rfc3339("2025-01-01T00:00:00Z").unwrap();
        set_timestamp_datetime(&start_time);

        // Create a new login
        let new_session = LoginSession::login(&pool, "user1@example.com", "password", &Some("127.0.0.1".to_string())).await.unwrap();
        assert_eq!("user1", new_session.name);
        assert_eq!(Some(start_time), new_session.loggedin);
        assert_eq!(DateTime::parse_from_rfc3339("2025-01-08T00:00:00Z").unwrap(), new_session.expiry);

        // Advance by one hour and load the record with updating access time
        let verify_time = DateTime::parse_from_rfc3339("2025-01-01T01:00:00Z").unwrap();
        set_timestamp_datetime(&verify_time);
        let read_session = LoginSession::load(&pool, &new_session.sessionid).await.unwrap().unwrap();
        assert_eq!(new_session.sessionid, read_session.sessionid);
        assert_eq!("user1", read_session.name);
        assert_eq!(Some(start_time), read_session.loggedin);
        assert_eq!(DateTime::parse_from_rfc3339("2025-01-08T00:00:00Z").unwrap(), read_session.expiry);

    }

    async fn count_session_rows(pool: &PgPool) -> i64 {
        let count_record = query("SELECT COUNT(*) AS c FROM loginsession")
            .fetch_one(pool)
            .await.unwrap();
        count_record.try_get("c").unwrap()
    }

    fn rocket(pool: PgPool) -> Rocket<Build> {
        rocket::build()
            .manage(pool)
            .manage(user_agent_parser::UserAgentParser::from_path("user_agents.yaml").unwrap())
            .mount("/", routes![
                crate::loginsession::login,
                crate::loginsession::verify_session,
                crate::loginsession::logout,
                crate::loginsession::get_sessions,
                crate::loginsession::get_session_by_id,
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
            .cookie((SESSION_ID, login.sessionid_raw.clone()))
            .dispatch().await;
        assert_eq!(resp_verify_login.status(), Status::NoContent);
        assert_eq!(count_session_rows(&pool).await, 1, "should be 1 session after login/verify");
    
        // Logout
        let resp_logout = client.post(uri!(crate::loginsession::logout))
            .cookie((SESSION_ID, login.sessionid_raw.clone()))
            .dispatch().await;
        assert_eq!(resp_logout.status(), Status::NoContent);
        assert_eq!(count_session_rows(&pool).await, 0, "should be 0 sessions after logout");
        assert_eq!(resp_logout.cookies().get(SESSION_ID).unwrap().max_age().unwrap().whole_microseconds(), 0);
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
        assert_eq!(Some(7*24*60*60), login1.max_age_secs);
        let session_after_first_login = LoginSession::load(&pool, &login1.sessionid).await.unwrap().unwrap();

        // Advance clock by 1 hour
        set_timestamp_rfc3339("2025-01-01T01:00:00Z");

        // Login again with existing cookie => no new session record, existing session is extended
        let mut post = client.post(uri!(crate::loginsession::login))
            .cookie((SESSION_ID, login1.sessionid_raw.clone()));
        post.set_body(json!({
            "email": "user1@example.com",
            "password": "password"
        }).to_string());

        let resp = post.dispatch().await;
        assert_eq!(resp.status(), Status::Ok);
        assert_eq!(count_session_rows(&pool).await, 1, "should still be 1 session after login");
        let login2 = LoginCookie::from_response(&resp).unwrap();
        
        assert_eq!(login1.sessionid, login2.sessionid);
        let session_after_second_login = LoginSession::load(&pool, &login1.sessionid).await.unwrap().unwrap();

        let expiry_delta = session_after_second_login.expiry - session_after_first_login.expiry;
        assert_eq!(expiry_delta.num_seconds(), 3600, "cookie expiry should be extended by 1 hour");
    }
    
    #[sqlx::test(fixtures("../schema.sql", "fixtures/users.sql"))]
    fn dispatch_login_then_login_with_incorrect_password_deletes_session(pool: PgPool) {        
        let client = Client::untracked(rocket(pool.clone())).await.expect("valid rocket instance");
    
        assert_eq!(count_session_rows(&pool).await, 0, "should be 0 session initially");
    
        // Login
        let login1 = dispatch_login(&client, "user1@example.com", "password").await.unwrap();
        assert_eq!(count_session_rows(&pool).await, 1, "should be 1 session after login");
        assert_eq!(Some(7*24*60*60), login1.max_age_secs);

        // Login again with existing cookie but wrong password => deletes session record
        let mut post = client.post(uri!(crate::loginsession::login))
            .cookie((SESSION_ID, login1.sessionid_raw.clone()));
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
        assert_eq!(login2.max_age_secs, Some(0), "session cookie should have max_age zeroed to delete it from the browser");
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
        max_age_secs: Option<i64>
    }

    impl LoginCookie {
        fn from_response(resp: &LocalResponse) -> Option<Self> {
            resp.cookies().get(SESSION_ID).map(|cookie| {
                let cookie_private = resp.cookies()
                    .get_private(SESSION_ID)
                    .map(|c| c.value().to_string())
                    .unwrap_or_else(|| cookie.value().to_string());
                Self {
                    sessionid_raw: cookie.value().to_string(),
                    sessionid: cookie_private,
                    max_age_secs: cookie.max_age()
                        .map(|d| d.whole_seconds())
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
