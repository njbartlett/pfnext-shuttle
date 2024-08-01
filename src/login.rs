use std::fmt::format;
use std::num;
use std::ops::Add;

use chrono::{DateTime, Duration, Utc};
use mail_send::mail_builder::headers::address::{Address, EmailAddress};
use mail_send::mail_builder::MessageBuilder;
use mail_send::smtp::message::{IntoMessage, Message};
use mail_send::{Credentials, SmtpClientBuilder};
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

use crate::{AppState, CountResult};
use crate::claims::Claims;

const ACCESS_TOKEN_TTL: Duration = Duration::hours(3);
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

#[derive(Deserialize)]
pub struct LoginRequest {
    email: String,
    password: String,
}

#[derive(Responder)]
#[response(status = 200, content_type = "application/json")]
pub struct LoginResponse {
    inner: Json<LoggedInUser>,
    cookie: Header<'static>
}

#[derive(Serialize)]
pub struct LoggedInUser {
    id: i64,
    name: String,
    email: String,
    phone: Option<String>,
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

async fn verify_user_by_id(pool: &PgPool, user_id: i64, password: &str) -> Result<UserLoginRecord, Custom<String>> {
    verify_user(load_user_record_by_id(pool, user_id).await?.ok_or_else(|| Custom(Status::Unauthorized, INVALID_LOGIN_MESSAGE.to_string()))?, password)
}

async fn verify_user_by_email(pool: &PgPool, email: &str, password: &str) -> Result<UserLoginRecord, Custom<String>> {
    verify_user(load_user_record_by_email(pool, email).await?.ok_or_else(|| Custom(Status::Unauthorized, INVALID_LOGIN_MESSAGE.to_string()))?, password)
}

fn verify_user(login_record: UserLoginRecord, password: &str) -> Result<UserLoginRecord, Custom<String>> {
    let recorded_pwd = login_record.pwd
        .as_ref()
        .ok_or_else(|| Custom(Status::Forbidden, "please reset your password".to_string()))?;
    verify_password(password, &recorded_pwd)
        .map_err(|_| Custom(Status::Unauthorized, INVALID_LOGIN_MESSAGE.to_string()))?;

    Ok(login_record)
}

#[post("/login", data = "<login>")]
pub async fn login(state: &State<AppState>, login: Json<LoginRequest>) -> Result<LoginResponse, Custom<String>> {
    let login_record = verify_user_by_email(&state.pool, &login.email, &login.password).await?;
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
    let login_record = verify_user_by_email(&state.pool, &password_update.username, &password_update.current_password).await?;

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
    website_url: String,
    reset_url: String
}

#[derive(Serialize, FromRow, Debug)]
struct UserUpdated {
    id: i64
}

#[derive(Deserialize)]
pub struct PasswordResetRequest {
    email: String,
    website_url: String,
    reset_url: String
}

#[post("/request_pwd_reset", data="<reset_request>")]
pub async fn request_pwd_reset(
    state: &State<AppState>,
    reset_request: Json<PasswordResetRequest>
) -> Result<Accepted<String>, Custom<String>> {
    let user_record = load_user_record_by_email(&state.pool, &reset_request.email)
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
    let text = format!(include_str!("reset_email.txt"), &reset_request.website_url, temp_password, reset_url_with_params, TEMP_PASSWORD_EXPIRY.num_minutes());
    let sender = Address::new_address(Some(&state.config.email_sender_name), &state.config.email_sender_address);
    let message = MessageBuilder::new()
        .from(sender.clone())
        .reply_to(sender)
        .to(Address::new_address(Some(&user_record.name), &user_record.email))
        .subject(format!("Password Reset for {}", &state.config.branding))
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
    let existing_user_record = load_user_record_by_email(&state.pool, &new_user.email).await?;
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
    let text = format!(include_str!("register_email.txt"), &new_user.website_url, temp_password, reset_url_with_params, TEMP_PASSWORD_EXPIRY.num_minutes());
    let sender = Address::new_address(Some(&state.config.email_sender_name), &state.config.email_sender_address);
    let message = MessageBuilder::new()
        .from(sender.clone())
        .reply_to(sender)
        .to(Address::new_address(Some(&new_user.name), &new_user.email))
        .subject(format!("New User Registration for {}", &state.config.branding))
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
    new_password: String,
    website_url: String
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
    let user_record = load_user_record_by_email(&state.pool, &user_pwd_reset.email)
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

    // Send acknowledgement email
    let text = format!(include_str!("post_reset_email.txt"), &user_record.name, &user_record.email, &user_pwd_reset.website_url);
    let sender = Address::new_address(Some(&state.config.email_sender_name), &state.config.email_sender_address);
    let message = MessageBuilder::new()
        .from(sender.clone())
        .reply_to(sender)
        .to(Address::new_address(Some(&user_record.name), &user_record.email))
        .subject(format!("Password Changed for {}", &state.config.branding))
        .text_body(text)
        .into_message()
        .map_err(|e| Custom(Status::InternalServerError, e.to_string()))?;
    let _ = send_email(message, &state.secrets)
        .await
        .inspect_err(|e| error!("Failed to send password change email to {}: {:?}", &user_record.email, e));

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

    let mut users: Vec<User> = query_as("SELECT id, name, email, phone, roles FROM person ORDER BY name")
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

#[derive(Deserialize)]
pub struct UserDelete {
    password: Option<String>,
    website_url: String
}

#[delete("/users/<user_id>", data="<deletion>")]
pub async fn delete_user(state: &State<AppState>, claims: Claims, user_id: i64, deletion: Json<UserDelete>) -> Result<NoContent, Custom<String>> {
    // Load the user record
    let mut login_record = load_user_record_by_id(&state.pool, user_id)
        .await?
        .ok_or(Custom(Status::NotFound, format!("user id not found: {}", user_id)))?;

    if user_id == claims.uid {
        // If this is the current user, require correct password even if the user is an admin
        let password = deletion.password.as_ref().ok_or(Custom(Status::Forbidden, "password is required to delete profile".to_string()))?;
        login_record = verify_user(login_record, password)?;
    } else {
        // Not the current user, only admins can perform
        claims.assert_roles_contains("admin")?;
    }

    // Actually delete the data. Related records in bookings are removed by DELETE CASCADE
    let _ = query_as("DELETE FROM person WHERE id = $1 RETURNING id")
        .bind(user_id)
        .fetch_one(&state.pool)
        .await
        .map_err(|e| Custom(Status::InternalServerError, e.to_string()))?;

    // Send an email to the user confirming their account has been deleted
    let text = format!(include_str!("post_delete_profile_email.txt"), &login_record.email, &deletion.website_url);
    let sender = Address::new_address(Some(&state.config.email_sender_name), &state.config.email_sender_address);
    let message = MessageBuilder::new()
        .from(sender.clone())
        .reply_to(sender)
        .to(Address::new_address(Some(&login_record.name), &login_record.email))
        .subject(format!("User Profile Deleted for {}", &state.config.branding))
        .text_body(text)
        .into_message()
        .map_err(|e| Custom(Status::InternalServerError, e.to_string()))?;
    let _ = send_email(message, &state.secrets)
        .await
        .inspect_err(|e| error!("Failed to send deletion email to {}: {:?}", &login_record.email, e));

    Ok(NoContent)
}

#[derive(Deserialize)]
pub struct UserUpdate {
    name: String,
    email: String,
    phone: Option<String>,
    roles: Vec<String>
}

#[put("/users/<user_id>", data="<update>")]
pub async fn update_user(state: &State<AppState>, claims: Claims, user_id: i64, update: Json<UserUpdate>) -> Result<Accepted<String>, Custom<String>> {
    if !claims.uid == user_id {
        let _ = claims.assert_roles_contains("admin")?;
    }

    let roles_str = &update.roles.join(",");
    let _: UserLoginRecord = query_as("UPDATE person SET name = $1, email = $2, phone = $3, roles = $4 WHERE id = $5 RETURNING id, name, email, phone, pwd, roles")
        .bind(&update.name)
        .bind(&update.email)
        .bind(&update.phone)
        .bind(roles_str)
        .bind(user_id)
        .fetch_one(&state.pool)
        .await
        .map_err(|e| Custom(Status::InternalServerError, e.to_string()))?;

    Ok(Accepted(String::from("user updated")))
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

fn build_login_response(
    login_record: UserLoginRecord,
    secrets: &shuttle_runtime::SecretStore
) -> Result<LoginResponse, Custom<String>> {
    // Create access and refresh tokens
    let roles = parse_roles(&login_record.roles);
    let access_token_key = secrets.get("ACCESS_TOKEN_KEY")
        .ok_or(Custom(Status::InternalServerError, String::from("missing secret ACCESS_TOKEN_KEY")))?;
    let access_token = Claims::create(login_record.id, &login_record.email, &login_record.phone, &roles, ACCESS_TOKEN_TTL).into_token(&access_token_key)?;
    let refresh_token_key = secrets.get("REFRESH_TOKEN_KEY")
        .ok_or(Custom(Status::InternalServerError, String::from("missing secret REFRESH_TOKEN_KEY")))?;
    let refresh_token: String = Claims::create(login_record.id, &login_record.email, &login_record.phone, &roles, REFRESH_TOKEN_EXIRATION).into_token(&refresh_token_key)?;

    // Build login response body
    let body = LoggedInUser {
        id: login_record.id,
        name: login_record.name,
        email: login_record.email,
        phone: login_record.phone,
        roles,
        access_token
    };

    // Build overall response with refresh token as cookie
    let cookie_expiry = Utc::now().add(REFRESH_TOKEN_EXIRATION);
    Ok(LoginResponse {
        inner: Json(body),
        cookie: Header::new("Set-Cookie", format!("refresh_token={};HttpOnly;Expires={}", refresh_token, cookie_expiry.to_rfc2822()))
    })
}

async fn load_user_record_by_email(pool: &PgPool, user_email: &str) -> Result<Option<UserLoginRecord>, Custom<String>> {
    query_as("SELECT id, name, email, phone, pwd, roles FROM person WHERE email = $1")
        .bind(user_email)
        .fetch_optional(pool)
        .await
        .map_err(|e| Custom(Status::InternalServerError, e.to_string()))
}

async fn load_user_record_by_id(pool: &PgPool, user_id: i64) -> Result<Option<UserLoginRecord>, Custom<String>> {
    query_as("SELECT id, name, email, phone, pwd, roles FROM person WHERE id = $1")
        .bind(user_id)
        .fetch_optional(pool)
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
    let smtp_host = secrets.get("SMTP_HOST")
        .ok_or(Custom(Status::InternalServerError, "SMTP credentials not found".to_string()))?;
    let smtp_port: u16 = secrets.get("SMTP_HOST_PORT")
        .ok_or(Custom(Status::InternalServerError, "SMTP credentials not found".to_string()))?
        .parse::<u16>()
        .map_err(|e| Custom(Status::InternalServerError, format!("Failed to read SMTP port: {}", e.to_string())))?;

    // Open the client
    info!("Connecting to SMTP server at {}:{}...", smtp_host, smtp_port);
    let mut client = SmtpClientBuilder::new(smtp_host, smtp_port)
        .implicit_tls(true)
        .credentials(Credentials::new(smtp_username, smtp_password))
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
