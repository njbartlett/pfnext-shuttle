// main.rs
#[macro_use]
extern crate rocket;

use std::env;
use std::path::{Path, PathBuf};
use chrono::{DateTime, FixedOffset};

use rocket::Request;
use rocket::fs::NamedFile;
use rocket::fs::relative;
use rocket::http::{Method, Status};
use rocket::response::status::Custom;
use rocket::serde::Serialize;
use rocket_cors::{AllowedHeaders, AllowedOrigins};
use serde::Deserialize;
use shuttle_runtime::CustomError;
use sqlx::{Executor, FromRow, PgPool, query_as};
use crate::claims::AuthenticationError;
use crate::config::{AppEnv, Config};

mod claims;
mod config;
mod sessions;
mod login;
mod bookings;
mod backup;
mod log;

#[catch(403)]
pub fn forbidden(request: &Request) -> Custom<String> {
    let auth_error = request.local_cache::<Option<AuthenticationError>, _>(|| None);
    let message = match auth_error {
        Some(msg) => msg.to_string(),
        None      => "NOT AUTH".to_string()
    };
    Custom(Status::Forbidden, message)
}

#[shuttle_runtime::main]
async fn rocket(
    #[shuttle_shared_db::Postgres] pool: PgPool,
    #[shuttle_runtime::Secrets] secrets: shuttle_runtime::SecretStore
) -> shuttle_rocket::ShuttleRocket {
    // Initiate tables
    pool.execute(include_str!("../schema.sql"))
        .await
        .map_err(CustomError::new)?;

    // Init config
    let config: Config = Config::default();
    info!("Initialized config: {:?}", config);
    let app_env: AppEnv = AppEnv {
        database_url: secrets.get("DATABASE_URL").unwrap(),
        access_token_key: secrets.get("ACCESS_TOKEN_KEY").unwrap(),
        refresh_token_key: secrets.get("REFRESH_TOKEN_KEY").unwrap(),
        smtp_username: secrets.get("SMTP_USERNAME").unwrap(),
        smtp_password: secrets.get("SMTP_PASSWORD").unwrap(),
        cors_allowed: secrets.get("CORS_ALLOWED").unwrap(),
        static_path: secrets.get("STATIC_PATH").unwrap()
    };

    // Configure CORS
    let allow_domain = [&app_env.cors_allowed];
    let allowed_origins = AllowedOrigins::some_regex(&allow_domain);
    let cors = rocket_cors::CorsOptions {
        allowed_origins,
        allowed_methods: vec![Method::Get, Method::Post, Method::Options, Method::Head, Method::Delete, Method::Put, Method::Patch].into_iter().map(From::from).collect(),
        allowed_headers: AllowedHeaders::All,
        allow_credentials: true,
        ..Default::default()
    }.to_cors().map_err(CustomError::new)?;

    // Configure Rocket
    let rocket = rocket::build()
        .attach(cors)
        .manage(config)
        .manage(app_env)
        .manage(pool)
        .register("/", catchers![forbidden])
        .mount("/", routes![
            login::login, login::validate_login, login::change_password, login::register_user, login::request_pwd_reset, login::reset_pwd, login::get_user, login::list_users, login::delete_user, login::update_user, login::patch_user,
            sessions::list_sessions, sessions::get_session, sessions::create_session, sessions::delete_session,
            sessions::list_locations, sessions::list_session_types, sessions::update_session,
            bookings::list_bookings, bookings::create_booking, bookings::delete_booking, bookings::update_booking, bookings::get_attendance_stats,
            log::read_log,
            backup::backup_all
        ]);

    Ok(rocket.into())
}

#[derive(Serialize, FromRow, Clone, Debug)]
pub struct UserLoginRecord {
    id: i64,
    name: String,
    email: String,
    phone: Option<String>,
    pwd: Option<String>,
    roles: String,
    credits: i16
}

impl UserLoginRecord {
    pub async fn load_by_id(pool: &PgPool, user_id: i64) -> Result<Option<UserLoginRecord>, sqlx::Error> {
        query_as("SELECT id, name, email, phone, pwd, roles, credits FROM person WHERE id = $1")
            .bind(user_id)
            .fetch_optional(pool)
            .await
    }
    pub async fn load_by_email(pool: &PgPool, user_email: &str) -> Result<Option<UserLoginRecord>, sqlx::Error> {
        query_as("SELECT id, name, email, phone, pwd, roles, credits FROM person WHERE email = $1")
            .bind(user_email)
            .fetch_optional(pool)
            .await
    }
}

#[derive(FromRow, Serialize)]
struct BigintRecord {
    id: i64
}

#[derive(FromRow, Serialize, Clone, Debug, PartialEq)]
pub struct SessionType {
    id: i32,
    name: String,
    requires_trainer: bool,
    cost: i16,
    deprecated: bool
}

impl SessionType {
    async fn find_by_id(pool: &PgPool, id: i32) -> Result<Option<Self>, String> {
        query_as("SELECT * FROM session_type WHERE id = $1")
            .bind(id)
            .fetch_optional(pool)
            .await
            .map_err(|e| e.to_string())
    }
}

#[derive(Serialize, Clone, Debug)]
pub struct SessionTrainer {
    id: i64,
    name: String,
    email: String,
    url: Option<String>
}

#[derive(FromRow, Serialize, Clone, Debug, PartialEq)]
pub struct SessionLocation {
    id: i32,
    name: String,
    address: String,
    url: Option<String>
}

#[derive(FromRow, Debug)]
struct CountResult {
    count: i64
}

fn parse_opt_date(str: Option<String>) -> Result<Option<DateTime<FixedOffset>>, Custom<String>> {
    if str.is_none() {
        return Ok(None);
    }
    let parsed = DateTime::parse_from_rfc3339(str.as_ref().unwrap());
    println!("Parsed input {:?} to {:?}", &str, parsed);
    //.map_err(|e| BadRequest(e.to_string()))?;
    Ok(Some(parsed.map_err(|e| Custom(Status::UnprocessableEntity, e.to_string()))?))
}