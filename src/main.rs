// main.rs
#[macro_use]
extern crate rocket;

use std::env;
use std::path::{Path, PathBuf};
use chrono::{DateTime, FixedOffset};
use rand::Error;

use rand::prelude::*;
use rocket::{config, Request};
use rocket::fs::NamedFile;
use rocket::fs::relative;
use rocket::http::{Method, Status};
use rocket::response::Responder;
use rocket::response::status::Custom;
use rocket::serde::Serialize;
use rocket_cors::{AllowedHeaders, AllowedMethods, AllowedOrigins};
use serde::Deserialize;
use shuttle_runtime::CustomError;
use shuttle_runtime::Error::StringInterpolation;
use sqlx::{Executor, FromRow, PgPool};
use sqlx::migrate::MigrationSource;
use crate::claims::AuthenticationError;

mod claims;
mod sessions;
mod login;
mod bookings;
mod backup;

#[derive(Serialize, Deserialize, Debug)]
struct Config {
    branding: String,
    email_sender_name: String,
    email_sender_address: String,
    email_replyto_name: String,
    email_replyto_address: String,
    cors_allowed: String
}
impl ::std::default::Default for Config {
    fn default() -> Self {
        Self {
            branding: String::from("unbranded"),
            email_sender_name: String::from("Unknown"),
            email_sender_address: String::from("unknown@example.com"),
            email_replyto_name: String::from("Unknown"),
            email_replyto_address: String::from("unknown@example.com"),
            cors_allowed: String::from("^http://localhost")
        }
    }
}

struct AppState {
    pool: PgPool,
    secrets: shuttle_runtime::SecretStore,
    config: Config
}

#[rocket::get("/<path..>")]
pub async fn static_files(mut path: PathBuf) -> Option<NamedFile> {
    //path.set_extension("html");
    let mut path = Path::new(relative!("assets")).join(path);
    if path.is_dir() {
        path.push("index.html");
    }

    NamedFile::open(path).await.ok()
}

#[catch(403)]
pub fn forbidden(status: Status, request: &Request) -> Custom<String> {
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

    // Load config
    let mut config_path = env::current_dir()?;
    config_path.push("Config.properties");
    info!("Config path is {}", &config_path.display());
    let config: Config = confy::load_path(config_path).map_err(CustomError::new)?;
    info!("Loaded config: {:?}", config);

    // Configure CORS
    let allow_domain = [&config.cors_allowed];
    let allowed_origins = AllowedOrigins::some_regex(&allow_domain);
    let cors = rocket_cors::CorsOptions {
        allowed_origins,
        allowed_methods: vec![Method::Get, Method::Post, Method::Options, Method::Head, Method::Delete, Method::Put].into_iter().map(From::from).collect(),
        allowed_headers: AllowedHeaders::All,
        allow_credentials: true,
        ..Default::default()
    }.to_cors().map_err(CustomError::new)?;

    // Configure Rocket
    let state = AppState { pool, secrets, config };
    let rocket = rocket::build()
        .attach(cors)
        .register("/", catchers![forbidden])
        .mount("/", routes![
            static_files,
            login::login, login::validate_login, login::change_password, login::register_user, login::request_pwd_reset, login::reset_pwd, login::list_users, login::delete_user, login::update_user,
            sessions::list_sessions, sessions::get_session, sessions::create_session, sessions::delete_session,
            sessions::list_locations, sessions::list_session_types, sessions::update_session,
            bookings::list_bookings, bookings::create_booking, bookings::delete_booking, bookings::update_booking,
            backup::backup_all
        ])
        .manage(state);

    Ok(rocket.into())
}

#[derive(FromRow, Serialize)]
struct BigintRecord {
    id: i64
}

#[derive(FromRow, Serialize, Clone, Debug)]
pub struct SessionType {
    id: i32,
    name: String
}

#[derive(Serialize, Clone, Debug)]
pub struct SessionTrainer {
    id: i64,
    name: String,
    email: String
}

#[derive(FromRow, Serialize, Clone, Debug)]
pub struct SessionLocation {
    id: i32,
    name: String,
    address: String
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