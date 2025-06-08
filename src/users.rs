use std::ops::Add;
use std::time;

use chrono::{DateTime, Duration, Utc};
use mail_send::mail_builder::headers::address::Address;
use mail_send::mail_builder::MessageBuilder;
use mail_send::smtp::message::{IntoMessage, Message};
use mail_send::{Credentials, SmtpClientBuilder};
use password_auth::{generate_hash, verify_password};
use passwords::PasswordGenerator;
use rocket::http::{Header, Status};
use rocket::response::status::{Accepted, Custom, NoContent};
use rocket::serde::{Deserialize, Serialize};
use rocket::serde::json::Json;
use rocket::State;
use sqlx::{Error, FromRow, PgPool, query_as, raw_sql, Row, QueryBuilder, Postgres};
use sqlx::postgres::PgRow;
use urlencoding::encode;

use crate::loginsession::LoginSession;
use crate::{BigintRecord, CountResult, UserLoginRecord};
use crate::config::{Config, AppEnv};

const ACCESS_TOKEN_TTL: Duration = Duration::hours(6);
const ACCESS_TOKEN_TTL_ADMIN: Duration = Duration::hours(3);
const REFRESH_TOKEN_EXPIRATION: Duration = Duration::hours(24);

const PASSWORD_GENERATOR: PasswordGenerator = PasswordGenerator {
    length: 6,
    numbers: true,
    lowercase_letters: false,
    uppercase_letters: false,
    symbols: false,
    spaces: false,
    exclude_similar_characters: false,
    strict: false
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
    roles: Vec<String>,
    access_token: String
}

async fn verify_user_by_id(pool: &PgPool, user_id: i64, password: &str) -> Result<UserLoginRecord, Custom<String>> {
    let user_record = UserLoginRecord::load_by_id(pool, user_id)
        .await.map_err(|e| Custom(Status::InternalServerError, e.to_string()))?
        .ok_or_else(|| Custom(Status::Unauthorized, INVALID_LOGIN_MESSAGE.to_string()))?;
    verify_user(user_record, password)
}

async fn verify_user_by_email(pool: &PgPool, email: &str, password: &str) -> Result<UserLoginRecord, Custom<String>> {
    let user_record = UserLoginRecord::load_by_email(pool, email)
        .await.map_err(|e| Custom(Status::InternalServerError, e.to_string()))?
        .ok_or_else(|| Custom(Status::Unauthorized, INVALID_LOGIN_MESSAGE.to_string()))?;
    verify_user(user_record, password)
}

fn verify_user(login_record: UserLoginRecord, password: &str) -> Result<UserLoginRecord, Custom<String>> {
    let recorded_pwd = login_record.pwd
        .as_ref()
        .ok_or_else(|| Custom(Status::Forbidden, "please reset your password".to_string()))?;
    verify_password(password, &recorded_pwd)
        .map_err(|_| Custom(Status::Unauthorized, INVALID_LOGIN_MESSAGE.to_string()))?;

    Ok(login_record)
}

#[derive(Deserialize)]
pub struct UpdatePasswordRequest {
    username: String,
    current_password: String,
    new_password: String
}

#[derive(Deserialize, Debug)]
pub struct NewUserRequest {
    name: String,
    email: String,
    phone: Option<String>,
    emergency_name: Option<String>,
    emergency_phone: Option<String>,
    medical_info: Option<String>,
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
    state: &State<PgPool>,
    config: &State<Config>,
    app_env: &State<AppEnv>,
    reset_request: Json<PasswordResetRequest>
) -> Result<Accepted<String>, Custom<String>> {
    let user_record = UserLoginRecord::load_by_email(state.inner(), &reset_request.email)
        .await.map_err(|e| Custom(Status::InternalServerError, e.to_string()))?
        .ok_or(Custom(Status::BadRequest, format!("user does not exist: {}", reset_request.email)))?;

    // Fail if we have sent an email to this address within the last 2 mins
    let latest_previous_sent_time = Utc::now().add(TEMP_PASSWORD_MINIMUM_RESEND_WAIT);
    let latest_previous_sent_count: CountResult = query_as("SELECT count(*) FROM temp_password WHERE person_id = $1 AND sent > $2")
        .bind(&user_record.id)
        .bind(latest_previous_sent_time)
        .fetch_one(state.inner())
        .await
        .map_err(|e| Custom(Status::InternalServerError, e.to_string()))?;
    if latest_previous_sent_count.count > 0 {
        return Err(Custom(Status::BadRequest, format!("Cannot send another reset email within {} minutes.", TEMP_PASSWORD_MINIMUM_RESEND_WAIT.num_minutes().abs())));
    }

    // Create temp password and send
    let temp_password = create_temp_password(state.inner(), user_record.id).await?;
    let reset_url_with_params = format!("{}?email={}&temp_pwd={}", &reset_request.reset_url, encode(&user_record.email), encode(&temp_password));
    let text = format!(include_str!("reset_email.txt"), &config.inner().branding, temp_password, reset_url_with_params, TEMP_PASSWORD_EXPIRY.num_minutes());
    let sender = Address::new_address(Some(&config.inner().email_sender_name), &config.inner().email_sender_address);
    let message = MessageBuilder::new()
        .from(sender.clone())
        .reply_to(sender)
        .to(Address::new_address(Some(&user_record.name), &user_record.email))
        .subject(format!("Password Reset for {}", &config.inner().branding))
        .text_body(text)
        .into_message()
        .map_err(|e| Custom(Status::InternalServerError, e.to_string()))?;
    send_email(message, config, app_env).await?;

    Ok(Accepted(format!("Password reset email sent to {}. Please check your spam folder if not received!", &user_record.email)))
}

#[post("/register_user", data="<new_user>")]
pub async fn register_user(
    state: &State<PgPool>,
    config: &State<Config>,
    app_env: &State<AppEnv>,
    new_user: Json<NewUserRequest>
) -> Result<Accepted<String>, Custom<String>> {
    // Error if already existing record for the specified email
    let existing_user_record = UserLoginRecord::load_by_email(state.inner(), &new_user.email)
        .await.map_err(|e| Custom(Status::InternalServerError, e.to_string()))?;
    if let Some(_existing) = existing_user_record {
        return Err(Custom(Status::Conflict, "User already exists with this email address".to_string()));
    }

    // Create user record with null password (must use password reset)
    let user_updated: UserUpdated = query_as("INSERT INTO person (name, email, phone, emergency_name, emergency_phone, medical_info, credits, roles) VALUES ($1, $2, $3, $4, $5, $6, 1, '') RETURNING id")
        .bind(&new_user.name)
        .bind(&new_user.email)
        .bind(&new_user.phone)
        .bind(&new_user.emergency_name)
        .bind(&new_user.emergency_phone)
        .bind(&new_user.medical_info)
        .fetch_one(state.inner())
        .await
        .map_err(|e| Custom(Status::InternalServerError, e.to_string()))?;
    info!("Created new user id {} for {:?}", user_updated.id, &new_user);

    // Create temp password and send to email
    let temp_password = create_temp_password(state.inner(), user_updated.id).await?;
    let reset_url_with_params = format!("{}?email={}&temp_pwd={}", &new_user.reset_url, encode(&new_user.email), encode(&temp_password));
    let text = format!(include_str!("register_email.txt"), &config.branding, temp_password, reset_url_with_params, TEMP_PASSWORD_EXPIRY.num_minutes());
    let sender = Address::new_address(Some(&config.email_sender_name), &config.email_sender_address);
    let message = MessageBuilder::new()
        .from(sender.clone())
        .reply_to(sender.clone())
        .to(Address::new_address(Some(&new_user.name), &new_user.email))
        .subject(format!("New User Registration for {}", &config.branding))
        .text_body(text)
        .into_message()
        .map_err(|e| Custom(Status::InternalServerError, e.to_string()))?;
    send_email(message, config, app_env).await?;

    // Send notification email to admin
    let notification_message = MessageBuilder::new()
        .from(sender.clone())
        .reply_to(sender.clone())
        .to(config.email_admin_notifications.as_str())
        .subject(format!("New User Registration for {}", &config.branding))
        .text_body(format!(include_str!("register_notify_email.txt"),
            &new_user.name,
            &new_user.email,
            &new_user.phone.as_ref().unwrap_or(&"<unspecified>".to_string()),
            &new_user.emergency_name.as_ref().unwrap_or(&"<unspecified>".to_string()),
            &new_user.emergency_phone.as_ref().unwrap_or(&"<unspecified>".to_string()),
            &new_user.medical_info.as_ref().unwrap_or(&"None provided".to_string())
        ))
        .into_message()
        .map_err(|e| Custom(Status::InternalServerError, e.to_string()))?;
    send_email(notification_message, config, app_env).await?;

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
    pwd: String,
    expiry: DateTime<Utc>
}

#[post("/reset_pwd", data="<user_pwd_reset>")]
pub async fn reset_pwd(
    state: &State<PgPool>,
    config: &State<Config>,
    app_env: &State<AppEnv>,
    user_pwd_reset: Json<UserPasswordReset>
) -> Result<Accepted<String>, Custom<String>> {
    verify_suitable_password(&user_pwd_reset.new_password, &user_pwd_reset.temp_password)?;

    // Get the user => error if not found
    let user_record = UserLoginRecord::load_by_email(state.inner(), &user_pwd_reset.email)
        .await.map_err(|e| Custom(Status::InternalServerError, e.to_string()))?
        .ok_or(Custom(Status::BadRequest, format!("User does not exist with email address {}", &user_pwd_reset.email)))?;

    // Get the temporary password record and verify against user input
    let temp_pwd_record: TempPasswordRecord = query_as("SELECT person_id, pwd, expiry FROM temp_password WHERE person_id = $1")
        .bind(&user_record.id)
        .fetch_one(state.inner())
        .await
        .map_err(|_e| Custom(Status::Forbidden, "Password reset has not been requested, or it has expired.".to_string()))?;
    if temp_pwd_record.expiry.lt(&Utc::now()) {
        return Err(Custom(Status::Forbidden, "Password reset has expired.".to_string()))
    }
    verify_password(&user_pwd_reset.temp_password, &temp_pwd_record.pwd)
        .map_err(|_e| Custom(Status::Forbidden, INVALID_LOGIN_MESSAGE.to_string()))?;

    // Update the user's main password
    let updated_user: UserUpdated = query_as("UPDATE person SET pwd = $1 WHERE id = $2 RETURNING id")
        .bind(generate_hash(&user_pwd_reset.new_password))
        .bind(user_record.id)
        .fetch_one(state.inner())
        .await
        .map_err(|e| Custom(Status::InternalServerError, e.to_string()))?;
    info!("Updated password for user id {}", updated_user.id);

    // Clean up the temporary password record
    let _ = query_as("DELETE FROM temp_password WHERE person_id = $1 RETURNING person_id AS id")
        .bind(&user_record.id)
        .fetch_one(state.inner())
        .await
        .map(|user_updated: UserUpdated| info!("Deleted temporary password for user {}", user_updated.id))
        .inspect_err(|e| error!("Failed to delete temporary password for user {}: {}", &user_record.email, e));

    // Send acknowledgement email
    let text = format!(include_str!("post_reset_email.txt"), &user_record.name, &user_record.email, &user_pwd_reset.website_url);
    let sender = Address::new_address(Some(&config.email_sender_name), &config.email_sender_address);
    let message = MessageBuilder::new()
        .from(sender.clone())
        .reply_to(sender)
        .to(Address::new_address(Some(&user_record.name), &user_record.email))
        .subject(format!("Password Changed for {}", &config.branding))
        .text_body(text)
        .into_message()
        .map_err(|e| Custom(Status::InternalServerError, e.to_string()))?;
    let _ = send_email(message, config, app_env)
        .await
        .inspect_err(|e| error!("Failed to send password change email to {}: {:?}", &user_record.email, e));

    Ok(Accepted(format!("Updated password for user with email {}", &user_record.email)))
}

#[derive(Serialize, Debug, PartialEq)]
pub struct UserListingEntry {
    id: i64,
    name: String,
    email: String,
    phone: Option<String>,
    emergency_name: Option<String>,
    emergency_phone: Option<String>,
    medical_info: Option<String>,
    roles: Vec<String>,
    credits: i16,
    pwd_defined: bool
}

impl FromRow<'_, PgRow> for UserListingEntry {
    fn from_row(row: &PgRow) -> Result<Self, Error> {
        Ok(UserListingEntry {
            id: row.try_get("id")?,
            name: row.try_get("name")?,
            email: row.try_get("email")?,
            phone: row.try_get("phone").ok(),
            emergency_name: row.try_get("emergency_name").ok(),
            emergency_phone: row.try_get("emergency_phone").ok(),
            medical_info: row.try_get("medical_info").ok(),
            roles: parse_roles(row.try_get("roles")?),
            credits: row.try_get("credits")?,
            pwd_defined: row.try_get("pwd_defined")?
        })
    }
}

async fn query_users(pool: &PgPool, user_id: Option<i64>) -> Result<Vec<UserListingEntry>, Custom<String>> {
    let mut qb: QueryBuilder<Postgres> = Default::default();
    qb.push("SELECT id, name, email, phone, emergency_name, emergency_phone, medical_info, roles, credits, \
            (CASE WHEN pwd IS NULL THEN false ELSE true END) AS pwd_defined \
            FROM person");

    if let Some(user_id) = user_id {
        qb.push(" WHERE id = ");
        qb.push_bind(user_id);
    }
    qb.push(" ORDER BY name");

    qb.build_query_as()
        .fetch_all(pool)
        .await
        .map_err(|e| {
            error!("Failed to fetch user listing: {}", e);
            Custom(Status::InternalServerError, e.to_string())
        })
}

#[get("/users/<user_id>")]
pub async fn get_user(
    pool: &State<PgPool>,
    login: LoginSession,
    user_id: i64
) -> Result<Json<UserListingEntry>, Custom<String>> {
    if !login.is_admin() && !login.uid == user_id {
        return Err(Custom(Status::Forbidden, "cannot view user record for other users".to_string()));
    }
    let res = query_users(pool.inner(), Some(user_id))
        .await?
        .into_iter().next()
        .ok_or(Custom(Status::NotFound, format!("no user found for id {}", user_id)))?;
    Ok(Json(res))
}

#[get("/users/list?<role>")]
pub async fn list_users(
    pool: &State<PgPool>,
    login: LoginSession,
    role: Option<String>
) -> Result<Json<Vec<UserListingEntry>>, Custom<String>> {
    // If the user is not admin or trainer, list only returns the user
    let query_person_id = if login.is_admin() || login.has_role("trainer") {
        None
    } else {
        Some(login.uid)
    };

    let mut users = query_users(pool.inner(), query_person_id).await?;
    if let Some(filter_role) = role {
        users = users.into_iter()
            .filter(|u| u.roles.contains(&filter_role))
            .collect();
    }

    Ok(Json(users))
}

#[derive(Deserialize)]
pub struct UserDeletionRequest {
    password: Option<String>,
    website_url: String
}

#[delete("/users/<user_id>", data="<deletion>")]
pub async fn delete_user(
    state: &State<PgPool>,
    config: &State<Config>,
    app_env: &State<AppEnv>,
    login: LoginSession,
    user_id: i64,
    deletion: Json<UserDeletionRequest>
) -> Result<NoContent, Custom<String>> {
    // Load the user record
    let mut login_record = UserLoginRecord::load_by_id(state.inner(), user_id)
        .await.map_err(|e| Custom(Status::InternalServerError, e.to_string()))?
        .ok_or(Custom(Status::NotFound, format!("user id not found: {}", user_id)))?;

    if user_id == login.uid {
        // If this is the current user, require correct password even if the user is an admin
        let password = deletion.password.as_ref().ok_or(Custom(Status::Forbidden, "password is required to delete profile".to_string()))?;
        login_record = verify_user(login_record, password)?;
    } else {
        // Not the current user, only admins can perform
        if !login.is_admin() {
            return Err(Custom(Status::Forbidden, "admin role required".to_string()));
        }
    }

    // Actually delete the data. Related records in bookings are removed by DELETE CASCADE
    let _: BigintRecord = query_as("DELETE FROM person WHERE id = $1 RETURNING id")
        .bind(user_id)
        .fetch_one(state.inner())
        .await
        .map_err(|e| Custom(Status::InternalServerError, e.to_string()))?;

    // Send an email to the user confirming their account has been deleted
    let text = format!(include_str!("post_delete_profile_email.txt"), &login_record.email, &deletion.website_url);
    let sender = Address::new_address(Some(&config.email_sender_name), &config.email_sender_address);
    let message = MessageBuilder::new()
        .from(sender.clone())
        .reply_to(sender)
        .to(Address::new_address(Some(&login_record.name), &login_record.email))
        .subject(format!("User Profile Deleted for {}", &config.branding))
        .text_body(text)
        .into_message()
        .map_err(|e| Custom(Status::InternalServerError, e.to_string()))?;
    let _ = send_email(message, config, app_env)
        .await
        .inspect_err(|e| error!("Failed to send deletion email to {}: {:?}", &login_record.email, e));

    Ok(NoContent)
}

#[derive(Deserialize, Debug)]
pub struct UserUpdate {
    name: String,
    email: String,
    phone: Option<String>,
    emergency_name: Option<String>,
    emergency_phone: Option<String>,
    medical_info: Option<String>,
    roles: Vec<String>,
    credits: i32
}

#[put("/users/<user_id>", data="<update>")]
pub async fn update_user(
    state: &State<PgPool>,
    login: LoginSession,
    user_id: i64,
    update: Json<UserUpdate>
) -> Result<Accepted<String>, Custom<String>> {
    if !login.is_admin() && !login.uid == user_id {
        return Err(Custom(Status::Forbidden, "cannot edit user record for other users".to_string()));
    }

    let roles_str = &update.roles.join(",");
    let _: UserLoginRecord = query_as("UPDATE person SET name = $1, email = $2, phone = $3, emergency_name = $4, emergency_phone = $5, medical_info = $6, roles = $7, credits = $8 WHERE id = $9 RETURNING id, name, email, phone, pwd, roles, credits")
        .bind(&update.name)
        .bind(&update.email)
        .bind(&update.phone)
        .bind(&update.emergency_name)
        .bind(&update.emergency_phone)
        .bind(&update.medical_info)
        .bind(roles_str)
        .bind(&update.credits)
        .bind(user_id)
        .fetch_one(state.inner())
        .await
        .map_err(|e| Custom(Status::InternalServerError, e.to_string()))?;

    Ok(Accepted(String::from("user updated")))
}

#[derive(Deserialize, Debug)]
pub struct UserPatch {
    name: Option<String>,
    phone: Option<String>,
    emergency_name: Option<String>,
    emergency_phone: Option<String>,
    medical_info: Option<String>
}
#[patch("/users/<user_id>", data="<patch>")]
pub async fn patch_user(
    pool: &State<PgPool>,
    login: LoginSession,
    user_id: i64,
    patch: Json<UserPatch>
) -> Result<Accepted<String>, Custom<String>> {
    if !login.uid == user_id && !login.is_admin() {
        return Err(Custom(Status::Forbidden, "admin role required".to_string()));
    }

    let mut qb: QueryBuilder<Postgres> = QueryBuilder::new("UPDATE person");
    let mut operator = " SET ";
    if let Some(name) = &patch.name {
        qb.push(operator);
        qb.push("name = ");
        qb.push_bind(name);
        operator = ", ";
    }
    if let Some(phone) = &patch.phone {
        qb.push(operator);
        qb.push("phone = ");
        qb.push_bind(phone);
        operator = ", ";
    }
    if let Some(emergency_name) = &patch.emergency_name {
        qb.push(operator);
        qb.push("emergency_name = ");
        qb.push_bind(emergency_name);
        operator = ", ";
    }
    if let Some(emergency_phone) = &patch.emergency_phone {
        qb.push(operator);
        qb.push("emergency_phone = ");
        qb.push_bind(emergency_phone);
        operator = ", ";
    }
    if let Some(medical_info) = &patch.medical_info {
        qb.push(operator);
        qb.push("medical_info = ");
        qb.push_bind(medical_info);
        operator = ", ";
    }
    qb.push(" WHERE id = ");
    qb.push_bind(user_id);
    qb.push(" RETURNING id");

    if " SET ".eq(operator) {
        // No fields were set, don't execute the SQL
        return Ok(Accepted(String::from("no changes to user data")));
    }

    let _: BigintRecord = qb.build_query_as()
        .fetch_one(pool.inner())
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

async fn send_email<'x>(
    message: Message<'x>,
    config: &Config,
    app_env: &AppEnv
) -> Result<(), Custom<String>> {
    // Open the client
    info!("Connecting to SMTP server at {}:{}...", &app_env.smtp_host, &app_env.smtp_port);
    let mut client = SmtpClientBuilder::new(&app_env.smtp_host, app_env.smtp_port)
        .implicit_tls(true)
        .credentials(Credentials::new(&app_env.smtp_username, &app_env.smtp_password))
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

mod tests {
    use chrono::{Days, Duration, Utc};
    use rocket::http::Status;
    use rocket::response::status::Custom;
    use rocket::State;
    use sqlx::{FromRow, PgPool, query_as, Executor};
    use crate::users::{get_user, list_users};
    use crate::loginsession::LoginSession;

    const DEFAULT_PASSWORD: &str = "password";
    const DEFAULT_PASSWORD_HASH: Option<&str> = Some("$argon2id$v=19$m=19456,t=2,p=1$X6SS0kJdO6uW3snBe7t1hA$gcYt1rDiSi+f1Rh0tQK+xzgF6ou7zzEbY/2XW33z3YE");

    #[derive(FromRow)]
    struct BigintRecord {
        id: i64
    }
    async fn create_person(pool: &PgPool, email: &str, pwd: Option<&str>, roles: &str, credits: i32) -> i64 {
        let member_id: BigintRecord = query_as("insert into person (name, email, pwd, roles, credits) values ('Test User', $1, $2, $3, $4) returning id")
            .bind(email)
            .bind(pwd)
            .bind(roles)
            .bind(credits)
            .fetch_one(pool)
            .await.unwrap();
        member_id.id
    }

    fn create_login(name: &str, role: &str) -> LoginSession {
        LoginSession {
            sessionid: "xxx".to_string(),
            uid: 0,
            name: name.to_string(),
            email: format!("{}@example.com", name),
            roles: vec![role.to_string()],
            expiry: Utc::now().checked_add_days(Days::new(1)).unwrap().fixed_offset()
        }
    }

    #[sqlx::test]
    async fn verify_user_by_email(pool: PgPool) {
        pool.execute(include_str!("../schema.sql")).await.unwrap();

        let person_id = create_person(&pool, "joe@example.com", DEFAULT_PASSWORD_HASH, "member", 0).await;
        let verify_result = crate::users::verify_user_by_email(&pool, "joe@example.com", DEFAULT_PASSWORD).await.unwrap();
        assert_eq!(person_id, verify_result.id);
    }

    #[sqlx::test]
    async fn verify_user_by_id(pool: PgPool) {
        pool.execute(include_str!("../schema.sql")).await.unwrap();

        let person_id = create_person(&pool, "joe@example.com", DEFAULT_PASSWORD_HASH, "member", 0).await;
        let verify_result = crate::users::verify_user_by_id(&pool, person_id, DEFAULT_PASSWORD).await.unwrap();
        assert_eq!("joe@example.com", verify_result.email);
    }

    #[sqlx::test]
    async fn verify_user_by_id_incorrect_pwd(pool: PgPool) {
        pool.execute(include_str!("../schema.sql")).await.unwrap();

        let person_id = create_person(&pool, "joe@example.com", DEFAULT_PASSWORD_HASH, "member", 0).await;
        let verify_result = crate::users::verify_user_by_id(&pool, person_id, "wrong").await;
        assert_eq!(Custom(Status::Unauthorized, "incorrect username or password".to_string()), verify_result.err().unwrap());
    }

    #[sqlx::test]
    async fn get_users(pool: PgPool) {
        pool.execute(include_str!("../schema.sql")).await.unwrap();
        let pid1 = create_person(&pool, "joe@example.com", DEFAULT_PASSWORD_HASH, "member", 0).await;
        let pid2 = create_person(&pool, "bob@example.com", Option::None, "member", 0).await;

        let claim = create_login("admin", "admin");
        assert_eq!("joe@example.com", get_user(State::from(&pool), claim.clone(), pid1).await.unwrap().email);
        assert_eq!("bob@example.com", get_user(State::from(&pool), claim.clone(), pid2).await.unwrap().email);
        assert_eq!(Err(Custom(Status::NotFound, "no user found for id -1".to_string())), get_user(State::from(&pool), claim.clone(), -1).await);
    }

    #[sqlx::test]
    async fn list_users_admin(pool: PgPool) {
        pool.execute(include_str!("../schema.sql")).await.unwrap();
        create_person(&pool, "joe@example.com", None, "member", 0).await;
        create_person(&pool, "bob@example.com", None, "member", 0).await;

        let result = list_users(State::from(&pool), create_login("admin", "admin"), None).await.unwrap();

        assert_eq!(2, result.len());
    }

    #[sqlx::test]
    async fn list_users_nonadmin(pool: PgPool) {
        pool.execute(include_str!("../schema.sql")).await.unwrap();
        let user1 = create_person(&pool, "joe@example.com", None, "member", 0).await;
        let user2 = create_person(&pool, "bob@example.com", None, "member", 0).await;

        let claim = create_login("member", "member");
        let result = list_users(State::from(&pool), create_login("member", "member"), None).await.unwrap();

        assert_eq!(1, result.len());
        assert_eq!("joe@example.com", result[0].email);
    }

    #[sqlx::test]
    async fn list_users_with_pwd_defined_as_admin(pool: PgPool) {
        pool.execute(include_str!("../schema.sql")).await.unwrap();
        create_person(&pool, "joe@example.com", DEFAULT_PASSWORD_HASH, "member", 0).await;
        create_person(&pool, "bob@example.com", None, "member", 0).await;

        let result = list_users(State::from(&pool), create_login("admin", "admin"), None).await.unwrap();

        assert_eq!(2, result.len());
        assert_eq!(true, result.get(0).unwrap().pwd_defined);
        assert_eq!(false, result.get(1).unwrap().pwd_defined);
    }
}