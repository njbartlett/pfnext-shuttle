use std::fmt::format;
use std::num;
use std::ops::Add;

use chrono::{DateTime, Duration, Utc};
use mail_send::mail_builder::headers::address::{Address, EmailAddress};
use mail_send::mail_builder::MessageBuilder;
use mail_send::smtp::message::{IntoMessage, Message};
use mail_send::SmtpClientBuilder;
use password_auth::{generate_hash, verify_password};
use passwords::PasswordGenerator;
use rocket::http::{Header, Status};
use rocket::http::hyper::body::HttpBody;
use rocket::response::status::{Accepted, Custom, NoContent};
use rocket::serde::{Deserialize, Serialize};
use rocket::serde::json::Json;
use rocket::State;
use rocket::yansi::Paint;
use sqlx::{Error, FromRow, PgPool, query_as, QueryBuilder, raw_sql, Row};
use sqlx::postgres::PgRow;
use urlencoding::encode;

use crate::AppState;
use crate::claims::Claims;

const ACCESS_TOKEN_TTL: Duration = Duration::minutes(60);
const REFRESH_TOKEN_EXIRATION: Duration = Duration::days(1);

const PASSWORD_GENERATOR: PasswordGenerator = PasswordGenerator {
    length: 20,
    numbers: true,
    lowercase_letters: false,
    uppercase_letters: true,
    symbols: false,
    spaces: false,
    exclude_similar_characters: true,
    strict: true
};
const INVALID_LOGIN_MESSAGE: &str = "incorrect username or password";
const TEMP_PASSWORD_MINIMUM_RESEND_WAIT: Duration = Duration::minutes(-2);
const TEMP_PASSWORD_EXPIRY: Duration = Duration::minutes(10);
const EMAIL_SENDER_NAME: &str = "FitNext Admin";
const EMAIL_SENDER_ADDRESS: &str = "admin@fitnext.uk";
const EMAIL_REPLYTO_NAME: &str = "FitNext Admin";
const EMAIL_REPLYTO_ADDRESS: &str = "admin@fitnext.uk";

#[derive(Deserialize)]
pub struct LoginRequest {
    email: String,
    password: String,
}

#[derive(Responder)]
#[response(status = 200, content_type = "application/json")]
pub struct LoginResponse {
    inner: Json<LoggedInUser>,
    // cookie: Header<'static>
}

#[derive(Serialize)]
pub struct LoggedInUser {
    id: i64,
    name: String,
    email: String,
    roles: Vec<String>,
    access_token: String
}

#[derive(Serialize, FromRow, Clone, Debug)]
struct UserLoginRecord {
    id: i64,
    name: String,
    email: String,
    phone: Option<String>,
    pwd: Option<String>,
    roles: String
}

impl LoginResponse {
    pub(crate) fn from_logged_in_user(
        logged_in_user: LoggedInUser,
        secrets: &shuttle_runtime::SecretStore
    ) -> Result<Self, Custom<String>> {
        // let refresh_token = Claims::create(
        //     logged_in_user.id,
        //     &logged_in_user.email,
        //     &logged_in_user.roles,
        //     REFRESH_TOKEN_EXIRATION
        // ).into_token(secrets)?;
        let cookie_expiry = Utc::now().add(REFRESH_TOKEN_EXIRATION);
        Ok(Self {
            inner: Json(logged_in_user),
            // cookie: Header::new("Set-Cookie", format!("refresh_token={};HttpOnly;Expires={}", refresh_token, cookie_expiry.to_rfc2822()))
        })
    }
}

async fn verify_user(state: &State<AppState>, email: &str, password: &str) -> Result<UserLoginRecord, Custom<String>>{
    let login_record = load_user_record(state, email)
        .await?
        .ok_or_else(|| Custom(Status::Unauthorized, INVALID_LOGIN_MESSAGE.to_string()))?;
    info!("verify_user: loaded user record {:?}", login_record);
    let recorded_pwd = login_record.pwd
        .as_ref()
        .ok_or_else(|| Custom(Status::Forbidden, "please reset your password".to_string()))?;
    verify_password(password, &recorded_pwd)
        .map_err(|_| Custom(Status::Unauthorized, INVALID_LOGIN_MESSAGE.to_string()))?;

    Ok(login_record)
}

#[post("/login", data = "<login>")]
pub async fn login(state: &State<AppState>, login: Json<LoginRequest>) -> Result<LoginResponse, Custom<String>> {
    let login_record = verify_user(state, &login.email, &login.password).await?;
    build_login_response(login_record, &state.secrets)
}

#[get("/validate_login")]
pub async fn validate_login(claims: Claims) -> Result<NoContent, Custom<String>> {
    info!("Validated user login for user id {}, email {}", claims.uid, claims.email);
    Ok(NoContent)
}

#[derive(Deserialize)]
pub struct UpdatePasswordRequest {
    username: String,
    current_password: String,
    new_password: String
}

#[post("/change_password", data = "<password_update>")]
pub async fn change_password(state: &State<AppState>, password_update: Json<UpdatePasswordRequest>) -> Result<LoginResponse, Custom<String>> {
    let login_record = verify_user(state, &password_update.username, &password_update.current_password).await?;

    verify_suitable_password(&password_update.new_password, &password_update.current_password)?;

    // Update to new password and set must_change_pwd to false
    let pwd_hash = generate_hash(&password_update.new_password);
    query_as("UPDATE person SET pwd = $1, must_change_pwd = FALSE WHERE email = $2 RETURNING id")
        .bind(pwd_hash)
        .bind(&password_update.username)
        .fetch_optional(&state.pool)
        .await
        .map_err(|_| Custom(Status::Unauthorized, "Failed to update password".to_string()))?
        .ok_or(Custom(Status::NotFound, "No user updated".to_string()))?;

    build_login_response(login_record, &state.secrets)
}

#[derive(Deserialize, Debug)]
pub struct NewUserRequest {
    name: String,
    email: String,
    phone: Option<String>,
    reset_url: String
}

#[derive(Serialize, FromRow, Debug)]
struct UserUpdated {
    id: i64
}

#[derive(FromRow, Debug)]
struct CountResult {
    count: i64
}

#[derive(Deserialize)]
pub struct PasswordResetRequest {
    email: String,
    reset_url: String
}

#[post("/request_pwd_reset", data="<reset_request>")]
pub async fn request_pwd_reset(
    state: &State<AppState>,
    reset_request: Json<PasswordResetRequest>
) -> Result<Accepted<String>, Custom<String>> {
    let user_record = load_user_record(state, &reset_request.email)
        .await?
        .ok_or(Custom(Status::BadRequest, format!("user does not exist: {}", reset_request.email)))?;

    // Fail if we have sent an email to this address within the last 2 mins
    let latest_previous_sent_time = Utc::now().add(TEMP_PASSWORD_MINIMUM_RESEND_WAIT);
    let latest_previous_sent_count: CountResult = query_as("SELECT count(*) FROM temp_password WHERE person_id = $1 AND sent > $2")
        .bind(&user_record.id)
        .bind(latest_previous_sent_time)
        .fetch_one(&state.pool)
        .await
        .map_err(|e| Custom(Status::InternalServerError, e.to_string()))?;
    if latest_previous_sent_count.count > 0 {
        return Err(Custom(Status::BadRequest, format!("Cannot send another reset email within {} minutes.", TEMP_PASSWORD_MINIMUM_RESEND_WAIT.num_minutes().abs())));
    }

    // Create temp password and send
    let temp_password = create_temp_password(&state.pool, user_record.id).await?;
    let reset_url_with_params = format!("{}?email={}&temp_pwd={}", &reset_request.reset_url, encode(&user_record.email), encode(&temp_password));
    let text = format!(
        "You are receiving this email because you requested a password reset at fitnext.uk. To reset\n\
        your password, use the following temporary password on the password reset page:\n\
        \n\
            {}\n\
        \n\
        Alternatively click the following link or copy it into your web browser's address bar: \n\
        \n\
        {}\n\
        \n\
        This password and link will expire in {} minutes.\n\
        \n\
        If you did not request a password reset, you can safely ignore and delete this email.", temp_password, reset_url_with_params, TEMP_PASSWORD_EXPIRY.num_minutes());
    let message = MessageBuilder::new()
        .from((EMAIL_SENDER_NAME, EMAIL_SENDER_ADDRESS))
        .reply_to((EMAIL_REPLYTO_NAME, EMAIL_REPLYTO_ADDRESS))
        .to(Address::new_address(Some(&user_record.name), &user_record.email))
        .subject("💪 FitNext Password Reset")
        .text_body(text)
        .into_message()
        .map_err(|e| Custom(Status::InternalServerError, e.to_string()))?;
    send_email(message, &state.secrets).await?;

    Ok(Accepted(format!("Password reset email sent to {}. Please check your spam folder if not received!", &user_record.email)))
}

#[post("/register_user", data="<new_user>")]
pub async fn register_user(
    state: &State<AppState>,
    new_user: Json<NewUserRequest>
) -> Result<Accepted<String>, Custom<String>> {
    // Error if already existing record for the specified email
    let existing_user_record = load_user_record(state, &new_user.email).await?;
    if let Some(existing_user_record) = existing_user_record {
        return Err(Custom(Status::Conflict, "User already exists with this email address".to_string()));
    }

    // Create user record with null password (must use password reset)
    let user_updated: UserUpdated = query_as("INSERT INTO person (name, email, phone, roles) VALUES ($1, $2, $3, 'member') RETURNING id")
        .bind(&new_user.name)
        .bind(&new_user.email)
        .bind(&new_user.phone)
        .fetch_one(&state.pool)
        .await
        .map_err(|e| Custom(Status::InternalServerError, e.to_string()))?;
    info!("Created new user id {} for {:?}", user_updated.id, &new_user);

    // Create temp password and send to email
    let temp_password = create_temp_password(&state.pool, user_updated.id).await?;
    let reset_url_with_params = format!("{}?email={}&temp_pwd={}", &new_user.reset_url, encode(&new_user.email), encode(&temp_password));
    let text = format!(
        "You are receiving this email because you registered a new account on fitnext.uk.\n\
        To enable your account, please use the following temporary password on the password reset\n\
        page:\n\
        \n\
            {}\n\
        \n\
        Alternatively click the following link or copy it into your web browser's address bar: \n\
        \n\
        {}\n\
        \n\
        This password and link will expire in {} minutes.\n\
        \n\
        If you did not request new account, you can safely ignore and delete this email.", temp_password, reset_url_with_params, TEMP_PASSWORD_EXPIRY.num_minutes());
    let message = MessageBuilder::new()
        .from((EMAIL_SENDER_NAME, EMAIL_SENDER_ADDRESS))
        .reply_to((EMAIL_REPLYTO_NAME, EMAIL_REPLYTO_ADDRESS))
        .to(Address::new_address(Some(&new_user.name), &new_user.email))
        .subject("💪 FitNext User Registration")
        .text_body(text)
        .into_message()
        .map_err(|e| Custom(Status::InternalServerError, e.to_string()))?;
    send_email(message, &state.secrets).await?;

    Ok(Accepted(format!("New user instructions email sent to {}. Please check your spam folder if not received!", &new_user.email)))
}

async fn create_temp_password(pool: &PgPool, user_id: i64) -> Result<String, Custom<String>> {
    // Generate a temp password and expiry time
    let temp_password = PASSWORD_GENERATOR.generate_one()
        .map_err(|e| Custom(Status::InternalServerError, e.to_string()))?;
    let temp_password_hash = generate_hash(&temp_password);
    let now = Utc::now();
    let expiry_time = Utc::now().add(TEMP_PASSWORD_EXPIRY);

    // Insert or update record in temp_passwords
    let user_updated: UserUpdated = query_as(
        "INSERT INTO temp_password (person_id, pwd, sent, expiry) \
            VALUES ($1, $2, $3, $4) \
            ON CONFLICT (person_id) DO UPDATE SET pwd = $5, sent = $6, expiry = $7 \
            RETURNING person_id AS id")
        .bind(user_id)
        .bind(&temp_password_hash)
        .bind(&now)
        .bind(&expiry_time)
        .bind(&temp_password_hash)
        .bind(&now)
        .bind(&expiry_time)
        .fetch_one(pool)
        .await
        .map_err(|e| Custom(Status::BadRequest, e.to_string()))?;
    info!("Created temporary password for user with id {}", user_updated.id);

    // Since we are here, delete expired temp passwords
    let _ = raw_sql("DELETE FROM temp_password WHERE expiry < now()")
        .execute(pool)
        .await
        .inspect_err(|e| error!("Failed to clean temporary passwords table: {}", e));

    Ok(temp_password)
}


#[derive(Deserialize)]
pub struct UserPasswordReset {
    email: String,
    temp_password: String,
    new_password: String
}

#[derive(FromRow)]
struct TempPasswordRecord {
    person_id: i64,
    pwd: String,
    expiry: DateTime<Utc>
}

#[post("/reset_pwd", data="<user_pwd_reset>")]
pub async fn reset_pwd(
    state: &State<AppState>,
    user_pwd_reset: Json<UserPasswordReset>
) -> Result<Accepted<String>, Custom<String>> {
    verify_suitable_password(&user_pwd_reset.new_password, &user_pwd_reset.temp_password)?;

    // Get the user => error if not found
    let user_record = load_user_record(state, &user_pwd_reset.email)
        .await?
        .ok_or(Custom(Status::BadRequest, format!("User does not exist with email address {}", &user_pwd_reset.email)))?;

    // Get the temporary password record and verify against user input
    let temp_pwd_record: TempPasswordRecord = query_as("SELECT person_id, pwd, expiry FROM temp_password WHERE person_id = $1")
        .bind(&user_record.id)
        .fetch_one(&state.pool)
        .await
        .map_err(|e| Custom(Status::Forbidden, "Password reset has not been requested, or it has expired.".to_string()))?;
    verify_password(&user_pwd_reset.temp_password, &temp_pwd_record.pwd)
        .map_err(|e| Custom(Status::Forbidden, INVALID_LOGIN_MESSAGE.to_string()))?;

    // Update the user's main password
    let updated_user: UserUpdated = query_as("UPDATE person SET pwd = $1 WHERE id = $2 RETURNING id")
        .bind(generate_hash(&user_pwd_reset.new_password))
        .bind(user_record.id)
        .fetch_one(&state.pool)
        .await
        .map_err(|e| Custom(Status::InternalServerError, e.to_string()))?;
    info!("Updated password for user id {}", updated_user.id);

    // Clean up the temporary password record
    let _ = query_as("DELETE FROM temp_password WHERE person_id = $1 RETURNING person_id AS id")
        .bind(&user_record.id)
        .fetch_one(&state.pool)
        .await
        .map(|user_updated: UserUpdated| info!("Deleted temporary password for user {}", user_updated.id))
        .inspect_err(|e| error!("Failed to delete temporary password for user {}: {}", &user_record.email, e));
    Ok(Accepted(format!("Updated password for user with email {}", &user_record.email)))
}

#[derive(Serialize, Debug)]
pub struct User {
    id: i64,
    name: String,
    email: String,
    phone: Option<String>,
    roles: Vec<String>
}

impl FromRow<'_, PgRow> for User {
    fn from_row(row: &PgRow) -> Result<Self, Error> {
        Ok(User{
            id: row.try_get("id")?,
            name: row.try_get("name")?,
            email: row.try_get("email")?,
            phone: row.try_get("phone").ok(),
            roles: parse_roles(row.try_get("roles")?)
        })
    }
}

#[get("/users/list?<role>")]
pub async fn list_users(state: &State<AppState>, claim: Claims, role: Option<String>) -> Result<Json<Vec<User>>, Custom<String>> {
    if !claim.has_role("admin") {
        return Err(Custom(Status::Forbidden, "admin only".to_string()));
    }

    let mut users: Vec<User> = query_as("SELECT id, name, email, phone, roles FROM person")
        .fetch_all(&state.pool)
        .await
        .map_err(|e| Custom(Status::InternalServerError, e.to_string()))?;
    if let Some(filter_role) = role {
        users = users.into_iter()
            .filter(|u| u.roles.contains(&filter_role))
            .collect();
    }

    Ok(Json(users))
}

fn verify_suitable_password(new_password: &str, current_password: &str) -> Result<(), Custom<String>> {
    // Check suitability of new password
    if new_password.eq(current_password) {
        return Err(Custom(Status::Forbidden, "new password cannot be the same as the current password".to_string()));
    }
    if new_password.chars().count() < 8 {
        return Err(Custom(Status::Forbidden, "new password must be at least 8 characters in length".to_string()));
    }
    Ok(())
}

fn parse_roles(roles_str: &str) -> Vec<String> {
    roles_str.split(",").map(|s| s.to_string()).collect::<Vec<_>>()
}

fn build_login_response(
    login_record: UserLoginRecord,
    secrets: &shuttle_runtime::SecretStore
) -> Result<LoginResponse, Custom<String>> {
    // Create access token
    let roles = parse_roles(&login_record.roles);
    let access_token = Claims::create(login_record.id, &login_record.email, &roles, ACCESS_TOKEN_TTL).into_token(secrets)?;

    // Build login response body
    let body = LoggedInUser{
        id: login_record.id,
        name: login_record.name,
        email: login_record.email,
        roles,
        access_token
    };

    LoginResponse::from_logged_in_user(body, secrets)
}

async fn load_user_record(state: &State<AppState>, user_email: &str) -> Result<Option<UserLoginRecord>, Custom<String>> {
    query_as("SELECT id, name, email, phone, pwd, roles FROM person WHERE email = $1")
        .bind(user_email)
        .fetch_optional(&state.pool)
        .await
        .map_err(|e| Custom(Status::InternalServerError, e.to_string()))
}

async fn send_email<'x>(
    message: Message<'x>,
    secrets: &shuttle_runtime::SecretStore
) -> Result<(), Custom<String>> {
    // Make sure we have credentials to login
    let smtp_username = secrets.get("SMTP_USERNAME")
        .ok_or(Custom(Status::InternalServerError, "SMTP credentials not found".to_string()))?;
    let smtp_password = secrets.get("SMTP_PASSWORD")
        .ok_or(Custom(Status::InternalServerError, "SMTP credentials not found".to_string()))?;

    // Open the client
    info!("Connecting to SMTP server...");
    let mut client = SmtpClientBuilder::new("smtp.fastmail.com", 465)
        .implicit_tls(true)
        .credentials((smtp_username.as_str(), smtp_password.as_str()))
        .connect()
        .await
        .map_err(|e| Custom(Status::InternalServerError, e.to_string()))?;
    info!("Connected to SMTP server");

    // Send the message
    println!("Sending message: {:?}", message);
    client.send(message)
        .await
        .map_err(|e| Custom(Status::InternalServerError, e.to_string()))
}
