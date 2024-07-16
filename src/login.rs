use std::ops::Add;
use chrono::{DateTime, Duration, Utc};
use itertools::Itertools;
use password_auth::{generate_hash, verify_password};
use rocket::http::{Header, Status};
use rocket::response::status::Custom;
use rocket::serde::{Deserialize, Serialize};
use rocket::serde::json::{Json, to_string};
use rocket::State;
use sqlx::{FromRow, query_as};
use crate::AppState;
use crate::claims::Claims;

const ACCESS_TOKEN_TTL: Duration = Duration::minutes(1);
const REFRESH_TOKEN_EXIRATION: Duration = Duration::days(1);

#[derive(Deserialize)]
struct LoginRequest {
    username: String,
    password: String,
}

#[derive(Serialize, FromRow, Clone, Debug)]
struct UserLoginRecord {
    id: i64,
    name: String,
    email: String,
    pwd: String,
    roles: String
}

#[derive(Serialize)]
struct LoggedInUser {
    id: i64,
    name: String,
    email: String,
    roles: Vec<String>,
    access_token: String
}

const INVALID_LOGIN_MESSAGE: &str = "incorrect username or password";

#[derive(Responder)]
#[response(status = 200, content_type = "application/json")]
struct LoginResponse {
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

#[post("/login", data = "<login>")]
pub async fn login(state: &State<AppState>, login: Json<LoginRequest>) -> Result<LoginResponse, Custom<String>> {
    // Find user and verify password
    let login_record: UserLoginRecord = query_as("SELECT id, name, email, pwd, roles FROM person WHERE email = $1")
        .bind(&login.username)
        .fetch_one(&state.pool)
        .await
        .map_err(|e| Custom(Status::Unauthorized, INVALID_LOGIN_MESSAGE.to_string()))?;
    verify_password(&login.password, &login_record.pwd)
        .map_err(|e| Custom(Status::Unauthorized, INVALID_LOGIN_MESSAGE.to_string()))?;
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

#[derive(Deserialize, Debug)]
struct NewUser {
    name: String,
    email: String,
    pwd: String,
    phone: Option<String>,
    roles: Vec<String>
}

#[derive(Serialize, FromRow, Debug)]
struct NewUserResponse {
    id: i64
}

#[derive(Responder)]
#[response(status = 500)]
struct MyInternalError {
    text: String
}

#[post("/create_user", data="<user>")]
pub async fn create_user(state: &State<AppState>, user: Json<NewUser>) -> Result<Json<NewUserResponse>, MyInternalError> {
    let pwd_hash = generate_hash(&user.pwd);
    let roles_str = user.roles.join(",");
    let new_user_response: NewUserResponse = query_as("INSERT INTO person (name, email, phone, pwd, roles) \
            VALUES ($1, $2, $3, $4, $5)\
            RETURNING id")
        .bind(&user.name)
        .bind(&user.email)
        .bind(&user.phone)
        .bind(pwd_hash)
        .bind(roles_str)
        .fetch_one(&state.pool)
        .await
        .map_err(|e| MyInternalError{
            text: e.to_string()
        })?;
    Ok(Json(new_user_response))
}
