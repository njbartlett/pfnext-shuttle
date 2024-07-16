use std::ops::Add;

use chrono::{Duration, Utc};
use mail_send::mail_builder::MessageBuilder;
use mail_send::SmtpClientBuilder;
use password_auth::{generate_hash, verify_password};
use rocket::http::{Header, Status};
use rocket::response::status::Custom;
use rocket::serde::{Deserialize, Serialize};
use rocket::serde::json::Json;
use rocket::State;
use sqlx::{FromRow, query_as};

use crate::AppState;
use crate::claims::Claims;

const ACCESS_TOKEN_TTL: Duration = Duration::minutes(10);
const REFRESH_TOKEN_EXIRATION: Duration = Duration::days(1);

#[derive(Deserialize)]
pub struct LoginRequest {
    username: String,
    password: String,
}

#[derive(Serialize, FromRow, Clone, Debug)]
struct UserLoginRecord {
    id: i64,
    name: String,
    email: String,
    pwd: String,
    must_change_pwd: bool,
    roles: String
}

#[derive(Serialize)]
pub struct LoggedInUser {
    id: i64,
    name: String,
    email: String,
    roles: Vec<String>,
    access_token: String
}

const INVALID_LOGIN_MESSAGE: &str = "incorrect username or password";

#[derive(Responder)]
#[response(status = 200, content_type = "application/json")]
pub struct LoginResponse {
    inner: Json<LoggedInUser>,
    cookie: Header<'static>
}

impl LoginResponse {
    pub(crate) fn from_logged_in_user(logged_in_user: LoggedInUser,) -> Result<Self, Custom<String>> {
        let refresh_token = Claims::create(logged_in_user.id, &logged_in_user.email, &logged_in_user.roles, REFRESH_TOKEN_EXIRATION).into_token()?;
        let cookie_expiry = Utc::now().add(REFRESH_TOKEN_EXIRATION);
        Ok(Self {
            inner: Json(logged_in_user),
            cookie: Header::new("Set-Cookie", format!("refresh_token={};HttpOnly;Expires={}", refresh_token, cookie_expiry.to_rfc2822()))
        })
    }
}

fn build_login_response(login_record: UserLoginRecord) -> Result<LoginResponse, Custom<String>> {
    let roles = login_record.roles.split(",").map(|s| s.to_string()).collect::<Vec<_>>();

    // Create access token
    let access_token = Claims::create(login_record.id, &login_record.email, &roles, ACCESS_TOKEN_TTL).into_token()?;

    // Build login response body
    let body = LoggedInUser{
        id: login_record.id,
        name: login_record.name,
        email: login_record.email,
        roles, access_token
    };

    LoginResponse::from_logged_in_user(body)
}

async fn verify_user(state: &State<AppState>, username: &str, password: &str) -> Result<UserLoginRecord, Custom<String>>{
    let login_record: UserLoginRecord = query_as("SELECT id, name, email, pwd, must_change_pwd, roles FROM person WHERE email = $1")
        .bind(username)
        .fetch_one(&state.pool)
        .await
        .map_err(|_| Custom(Status::Unauthorized, INVALID_LOGIN_MESSAGE.to_string()))?;

    verify_password(password, &login_record.pwd)
        .map_err(|_| Custom(Status::Unauthorized, INVALID_LOGIN_MESSAGE.to_string()))?;

    Ok(login_record)
}

#[post("/login", data = "<login>")]
pub async fn login(state: &State<AppState>, login: Json<LoginRequest>) -> Result<LoginResponse, Custom<String>> {
    let login_record = verify_user(state, &login.username, &login.password).await?;
    if login_record.must_change_pwd {
        return Err(Custom(Status::Forbidden, "must change password".to_string()));
    }
    build_login_response(login_record)
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

    // Check suitability of new password
    if password_update.new_password.eq(&password_update.current_password) {
        return Err(Custom(Status::Forbidden, "new password cannot be the same as old password".to_string()));
    }
    if password_update.new_password.chars().count() < 8 {
        return Err(Custom(Status::Forbidden, "new password must be at least 8 characters in length".to_string()));
    }

    // Update to new password and set must_change_pwd to false
    let pwd_hash = generate_hash(&password_update.new_password);
    query_as("UPDATE person SET pwd = $1, must_change_pwd = FALSE WHERE email = $2 RETURNING id")
        .bind(pwd_hash)
        .bind(&password_update.username)
        .fetch_optional(&state.pool)
        .await
        .map_err(|_| Custom(Status::Unauthorized, "Failed to update password".to_string()))?
        .ok_or(Custom(Status::NotFound, "No user updated".to_string()))?;

    build_login_response(login_record)
}

#[derive(Deserialize, Debug)]
pub struct NewUser {
    name: String,
    email: String,
    pwd: String,
    phone: Option<String>,
    roles: Vec<String>
}

#[derive(Serialize, FromRow, Debug)]
pub struct NewUserResponse {
    id: i64
}

#[post("/create_user", data="<user>")]
pub async fn create_user(state: &State<AppState>, claims: Claims, user: Json<NewUser>) -> Result<Json<NewUserResponse>, Custom<String>> {
    if !claims.roles.contains(&"admin".to_string()) {
        return Err(Custom(Status::Forbidden, "only admins can create users".to_string()))
    }

    let pwd_hash = generate_hash(&user.pwd);
    let roles_str = user.roles.join(",");
    let new_user_response: NewUserResponse = query_as("INSERT INTO person (name, email, phone, pwd, roles, must_change_pwd) \
            VALUES ($1, $2, $3, $4, $5, TRUE) \
            RETURNING id")
        .bind(&user.name)
        .bind(&user.email)
        .bind(&user.phone)
        .bind(pwd_hash)
        .bind(roles_str)
        .fetch_one(&state.pool)
        .await
        .map_err(|e| Custom(Status::InternalServerError, e.to_string()))?;
    Ok(Json(new_user_response))
}

#[post("/register_user", data="<new_user>")]
pub async fn register_user(state: &State<AppState>, /*claims: Claims,*/ new_user: Json<NewUser>)  -> Result<Json<NewUserResponse>, Custom<String>> {
    // Build a simple multipart message
    let message = MessageBuilder::new()
        .from(("FitNext Admin (Neil Bartlett)", "nbartlett+fitnext@fastmail.net"))
        .reply_to(("FitNext Admin", "admin@fitnext.uk"))
        .to(vec![
            ("Neil Bartlett", "njbartlett@gmail.com"),
        ])
        .subject("Testing")
        .html_body("<h1>Hello, world!</h1>")
        .text_body("Hello world!");
    println!("Constructed message: {:?}", message);

    // Connect to the SMTP submissions port, upgrade to TLS and
    // authenticate using the provided credentials.
    let smtp_username = state.secrets.get("SMTP_USERNAME")
        .ok_or(Custom(Status::InternalServerError, "SMTP credentials not found".to_string()))?;
    let smtp_password = state.secrets.get("SMTP_PASSWORD")
        .ok_or(Custom(Status::InternalServerError, "SMTP credentials not found".to_string()))?;
    let mut client = SmtpClientBuilder::new("smtp.fastmail.com", 465)
        .implicit_tls(true)
        .credentials((smtp_username.as_str(), smtp_password.as_str()))
        .connect()
        .await
        .map_err(|e| Custom(Status::InternalServerError, e.to_string()))?;
        //
        // .unwrap()
        // .send(message)
        // .await
        // .unwrap();
    println!("Connected to client!");
    client.send(message)
        .await
        .map_err(|e| Custom(Status::InternalServerError, e.to_string()))?;
    println!("Sent it?!?");

    Err(Custom(Status::ServiceUnavailable, "not implemented".to_string()))
}