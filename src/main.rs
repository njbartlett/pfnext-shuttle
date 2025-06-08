// main.rs
#[macro_use]
extern crate rocket;

use std::collections::HashSet;
use chrono::{DateTime, FixedOffset};

use rocket::Request;
use rocket::http::{Method, Status};
use rocket::response::status::Custom;
use rocket::serde::Serialize;
use rocket_cors::{AllowedHeaders, AllowedOrigins};
use shuttle_runtime::CustomError;
use sqlx::{Executor, FromRow, PgPool, query_as};
use user_agent_parser::UserAgentParser;
use crate::config::{AppEnv, Config};
use crate::loginsession::AuthenticationError;

mod config;
mod sessions;
mod users;
mod bookings;
mod backup;
mod log;
mod activities;
mod whereclause;
mod loginsession;
mod mock_chrono;

#[catch(401)]
pub fn unauthorized(request: &Request) -> Custom<String> {
    let auth_error = request.local_cache::<Option<AuthenticationError>, _>(|| None);
    info!("### Intercepted 401 return, auth error: {:?}", auth_error);
    match auth_error {
        Some(err) => match err {
            AuthenticationError::MissingSession => Custom(Status::Unauthorized, "Your login session has expired".to_string()),
            AuthenticationError::MissingDatabase => Custom(Status::InternalServerError, "Failed to validate session".to_string()),
            AuthenticationError::DatabaseError(err) => Custom(Status::InternalServerError, err.to_string()),
        },
        None => Custom(Status::InternalServerError, "Failed to authenticate user".to_string())
    }
}

#[catch(404)]
pub fn notfound(request: &Request) -> Custom<String> {
    Custom(Status::NotFound, "not found".to_string())
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
        smtp_host: secrets.get("SMTP_HOST").unwrap(),
        smtp_port: secrets.get("SMTP_PORT").unwrap().parse().unwrap(),
        smtp_username: secrets.get("SMTP_USERNAME").unwrap(),
        smtp_password: secrets.get("SMTP_PASSWORD").unwrap(),
        cors_allowed: secrets.get("CORS_ALLOWED").unwrap(),
        static_path: secrets.get("STATIC_PATH").unwrap()
    };

    // Configure CORS
    info!("Initializing CORS with allowed origin regex: {}", &app_env.cors_allowed);
    let allow_domain = [&app_env.cors_allowed];
    let allowed_origins = AllowedOrigins::some_regex(&allow_domain);
    let cors = rocket_cors::CorsOptions {
        allowed_origins,
        allowed_methods: vec![Method::Get, Method::Post, Method::Options, Method::Head, Method::Delete, Method::Put, Method::Patch].into_iter().map(From::from).collect(),
        allowed_headers: AllowedHeaders::All,
        expose_headers: HashSet::from(["Location".to_string()]),
        allow_credentials: true,
        ..Default::default()
    }.to_cors().map_err(CustomError::new)?;

    // Configure Rocket
    let rocket = rocket::build()
        .attach(cors)
        .manage(config)
        .manage(app_env)
        .manage(pool)
        .register("/", catchers![unauthorized, notfound]) // TODO forbidden
        .mount("/", routes![
            loginsession::login, loginsession::logout, loginsession::verify_session,

            users::register_user, users::request_pwd_reset, users::reset_pwd, users::get_user, users::list_users, users::delete_user, users::update_user, users::patch_user,

            sessions::list_sessions, sessions::get_session, sessions::create_session, sessions::delete_session, sessions::list_locations, sessions::list_session_types, sessions::update_session,
            bookings::list_bookings, bookings::create_booking, bookings::delete_booking, bookings::update_booking, bookings::get_attendance_stats,

            activities::list_activity_types, activities::list_challenges, activities::get_challenge, activities::get_activity, activities::list_activities, activities::create_activity, activities::delete_activity,

            log::read_log, backup::backup_all
        ]);

    Ok(rocket.into())
}

#[derive(FromRow, Serialize, Debug)]
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
    Ok(Some(parsed.map_err(|e| Custom(Status::UnprocessableEntity, e.to_string()))?))
}